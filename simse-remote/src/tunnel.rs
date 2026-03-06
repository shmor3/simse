use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::error::RemoteError;
use crate::heartbeat::{Backoff, BackoffConfig, PING_INTERVAL_MS};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// Tunnel state visible to the server.
#[derive(Debug, Clone)]
pub struct TunnelState {
    pub connected: bool,
    pub tunnel_id: Option<String>,
    pub relay_url: Option<String>,
    pub connected_at: Option<Instant>,
    pub reconnect_count: u32,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            connected: false,
            tunnel_id: None,
            relay_url: None,
            connected_at: None,
            reconnect_count: 0,
        }
    }
}

/// Manages the WebSocket tunnel to the relay.
pub struct TunnelClient {
    state: Arc<Mutex<TunnelState>>,
    sink: Arc<Mutex<Option<WsSink>>>,
    connected: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl Default for TunnelClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TunnelState::default())),
            sink: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub async fn get_state(&self) -> TunnelState {
        self.state.lock().await.clone()
    }

    /// Connect to the relay WebSocket endpoint.
    pub async fn connect(
        &self,
        relay_url: &str,
        token: &str,
    ) -> Result<String, RemoteError> {
        if self.connected.load(Ordering::SeqCst) {
            return Err(RemoteError::TunnelAlreadyConnected);
        }

        let url = format!("{relay_url}/ws/tunnel?token={token}");
        let (ws_stream, _response) = connect_async(&url)
            .await
            .map_err(|e| RemoteError::ConnectionFailed(e.to_string()))?;

        let tunnel_id = uuid::Uuid::new_v4().to_string();
        let (sink, source) = ws_stream.split();

        *self.sink.lock().await = Some(sink);
        self.connected.store(true, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);

        {
            let mut state = self.state.lock().await;
            state.connected = true;
            state.tunnel_id = Some(tunnel_id.clone());
            state.relay_url = Some(relay_url.to_string());
            state.connected_at = Some(Instant::now());
        }

        // Spawn reader task
        let connected = self.connected.clone();
        let state = self.state.clone();
        let cancel = self.cancel.clone();
        let sink_ref = self.sink.clone();
        let relay_url_owned = relay_url.to_string();
        let token_owned = token.to_string();

        tokio::spawn(async move {
            Self::reader_loop(
                source,
                sink_ref,
                connected,
                state,
                cancel,
                relay_url_owned,
                token_owned,
            )
            .await;
        });

        tracing::info!("Tunnel connected: {tunnel_id}");
        Ok(tunnel_id)
    }

    /// Disconnect the tunnel.
    pub async fn disconnect(&self) -> Result<(), RemoteError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(RemoteError::TunnelNotConnected);
        }

        self.cancel.store(true, Ordering::SeqCst);
        self.connected.store(false, Ordering::SeqCst);

        // Close the WebSocket
        if let Some(mut sink) = self.sink.lock().await.take() {
            let _ = sink.close().await;
        }

        {
            let mut state = self.state.lock().await;
            state.connected = false;
            state.tunnel_id = None;
            state.connected_at = None;
        }

        tracing::info!("Tunnel disconnected");
        Ok(())
    }

    /// Send a message through the tunnel.
    pub async fn send_message(&self, msg: &str) -> Result<(), RemoteError> {
        let mut sink_guard = self.sink.lock().await;
        let sink = sink_guard
            .as_mut()
            .ok_or(RemoteError::TunnelNotConnected)?;
        sink.send(Message::Text(msg.to_string().into()))
            .await
            .map_err(|e| RemoteError::WebSocket(e.to_string()))?;
        Ok(())
    }

    /// Reader loop: receives messages from relay, handles pings, reconnection.
    async fn reader_loop(
        mut source: WsSource,
        sink: Arc<Mutex<Option<WsSink>>>,
        connected: Arc<AtomicBool>,
        state: Arc<Mutex<TunnelState>>,
        cancel: Arc<AtomicBool>,
        relay_url: String,
        token: String,
    ) {
        let mut backoff = Backoff::new(BackoffConfig::default());
        let mut ping_interval =
            tokio::time::interval(std::time::Duration::from_millis(PING_INTERVAL_MS));

        loop {
            if cancel.load(Ordering::SeqCst) {
                break;
            }

            tokio::select! {
                msg = source.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            tracing::debug!("Received from relay: {}", &text[..text.len().min(200)]);
                            // TODO: Route to local simse-core
                        }
                        Some(Ok(Message::Ping(data))) => {
                            if let Some(ref mut s) = *sink.lock().await {
                                let _ = s.send(Message::Pong(data)).await;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            tracing::warn!("WebSocket closed, attempting reconnect");
                            connected.store(false, Ordering::SeqCst);
                            state.lock().await.connected = false;

                            // Reconnect loop
                            loop {
                                if cancel.load(Ordering::SeqCst) {
                                    return;
                                }
                                let delay = backoff.next_delay();
                                tracing::info!("Reconnecting in {:?} (attempt {})", delay, backoff.attempts());
                                tokio::time::sleep(delay).await;

                                let url = format!("{relay_url}/ws/tunnel?token={token}");
                                match connect_async(&url).await {
                                    Ok((ws_stream, _)) => {
                                        let (new_sink, new_source) = ws_stream.split();
                                        *sink.lock().await = Some(new_sink);
                                        source = new_source;
                                        connected.store(true, Ordering::SeqCst);
                                        let mut s = state.lock().await;
                                        s.connected = true;
                                        s.reconnect_count += 1;
                                        s.connected_at = Some(Instant::now());
                                        backoff.reset();
                                        tracing::info!("Reconnected successfully");
                                        break;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Reconnect failed: {e}");
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {e}");
                        }
                        _ => {}
                    }
                }
                _ = ping_interval.tick() => {
                    if connected.load(Ordering::SeqCst) {
                        if let Some(ref mut s) = *sink.lock().await {
                            let _ = s.send(Message::Ping(vec![].into())).await;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_state_default() {
        let state = TunnelState::default();
        assert!(!state.connected);
        assert!(state.tunnel_id.is_none());
        assert_eq!(state.reconnect_count, 0);
    }

    #[test]
    fn tunnel_client_starts_disconnected() {
        let client = TunnelClient::new();
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn disconnect_fails_when_not_connected() {
        let client = TunnelClient::new();
        let result = client.disconnect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_fails_when_not_connected() {
        let client = TunnelClient::new();
        let result = client.send_message("test").await;
        assert!(result.is_err());
    }
}
