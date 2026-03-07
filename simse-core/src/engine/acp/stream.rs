// ---------------------------------------------------------------------------
// ACP Stream — streaming state machine for ACP generation responses
// ---------------------------------------------------------------------------
//
// `AcpStream` implements `futures::Stream<Item = StreamChunk>`. It represents
// an ongoing generation that yields chunks as they arrive from the agent
// subprocess via an mpsc channel.
//
// The connection's notification handler parses each `session/update`
// notification and sends parsed `StreamChunk` values into the channel.
// `AcpStream` polls the channel and yields chunks to the consumer.
//
// Features:
//   - Sliding-window timeout (resets on each chunk)
//   - Permission-aware timeout suspension
//   - External cancellation via CancellationToken
//   - Automatic termination on Complete chunk or channel close
// ---------------------------------------------------------------------------

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::engine::acp::client::TokenUsage;

// ---------------------------------------------------------------------------
// Streaming types (previously in protocol.rs)
// ---------------------------------------------------------------------------

/// Kind of tool call action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallKind {
	Read,
	Edit,
	Delete,
	Move,
	Search,
	Execute,
	Think,
	Fetch,
	Other,
}

/// Status of a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
	Pending,
	InProgress,
	Completed,
	Failed,
	Cancelled,
}

/// A tool call from a session/update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
	pub tool_call_id: String,
	pub title: String,
	pub kind: ToolCallKind,
	pub status: ToolCallStatus,
}

/// A tool call progress update from a session/update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallUpdate {
	pub tool_call_id: String,
	pub status: ToolCallStatus,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub content: Option<serde_json::Value>,
}

/// Discriminated union of streaming chunk types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
	/// Incremental text delta.
	Delta {
		text: String,
	},
	/// Stream completed with optional usage.
	Complete {
		#[serde(default, skip_serializing_if = "Option::is_none")]
		usage: Option<TokenUsage>,
	},
	/// A new tool call started.
	ToolCall {
		#[serde(rename = "toolCall")]
		tool_call: self::ToolCall,
	},
	/// Progress update on an existing tool call.
	ToolCallUpdate {
		update: self::ToolCallUpdate,
	},
}

// ---------------------------------------------------------------------------
// AcpStream
// ---------------------------------------------------------------------------

/// A streaming response from an ACP agent generation.
///
/// Implements [`futures::Stream`] yielding [`StreamChunk`] values. The stream
/// terminates when:
/// - A `Complete` chunk is received
/// - The mpsc channel is closed (sender dropped)
/// - The cancellation token is cancelled
/// - The sliding-window timeout fires (no chunk within `timeout_duration`)
pub struct AcpStream {
	/// Receives `StreamChunk` values from the notification handler.
	receiver: mpsc::Receiver<StreamChunk>,
	/// Sliding-window timeout duration. Resets on each received chunk.
	timeout_duration: Duration,
	/// Timestamp of the last received chunk (or stream creation).
	last_activity: Instant,
	/// When `true`, the timeout is suspended (permission prompt is active).
	permission_active: Arc<AtomicBool>,
	/// External cancellation token for aborting the stream.
	cancellation: CancellationToken,
	/// Whether the stream has completed (no more items will be yielded).
	completed: bool,
	/// Opaque handle(s) kept alive for the lifetime of the stream.
	/// Used to hold `SubscriptionHandle` so the notification handler
	/// remains active until the stream is consumed and dropped.
	_keep_alive: Vec<Box<dyn std::any::Any + Send>>,
}

impl AcpStream {
	/// Create a new `AcpStream`.
	///
	/// # Arguments
	///
	/// * `receiver` — channel receiving `StreamChunk` values from the
	///   notification handler.
	/// * `timeout_ms` — sliding-window timeout in milliseconds. The stream
	///   will emit a synthetic `Complete` chunk if no activity occurs within
	///   this window (unless permission is active).
	/// * `permission_active` — shared flag indicating a permission prompt is
	///   in progress, which suspends the timeout.
	/// * `cancellation` — token that, when cancelled, terminates the stream.
	pub fn new(
		receiver: mpsc::Receiver<StreamChunk>,
		timeout_ms: u64,
		permission_active: Arc<AtomicBool>,
		cancellation: CancellationToken,
	) -> Self {
		Self {
			receiver,
			timeout_duration: Duration::from_millis(timeout_ms),
			last_activity: Instant::now(),
			permission_active,
			cancellation,
			completed: false,
			_keep_alive: Vec::new(),
		}
	}

	/// Add handles that must be kept alive for the lifetime of the stream.
	///
	/// This is used to hold `SubscriptionHandle` instances so the
	/// notification handlers remain active while the stream is alive.
	///
	/// Uses owned-return pattern: takes `self` by value and returns the
	/// updated stream.
	pub fn keep_alive(mut self, handle: Box<dyn std::any::Any + Send>) -> Self {
		self._keep_alive.push(handle);
		self
	}
}

impl Stream for AcpStream {
	type Item = StreamChunk;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let this = self.get_mut();

		// 1. Already completed — no more items.
		if this.completed {
			return Poll::Ready(None);
		}

		// 2. External cancellation.
		if this.cancellation.is_cancelled() {
			this.completed = true;
			return Poll::Ready(None);
		}

		// 3. Sliding-window timeout (only when permission is NOT active).
		if !this.permission_active.load(Ordering::Relaxed)
			&& this.last_activity.elapsed() > this.timeout_duration
		{
			this.completed = true;
			return Poll::Ready(Some(StreamChunk::Complete { usage: None }));
		}

		// 4. Poll the receiver for the next chunk.
		match this.receiver.poll_recv(cx) {
			Poll::Ready(Some(chunk)) => {
				this.last_activity = Instant::now();
				if matches!(chunk, StreamChunk::Complete { .. }) {
					this.completed = true;
				}
				Poll::Ready(Some(chunk))
			}
			Poll::Ready(None) => {
				// Channel closed — sender dropped.
				this.completed = true;
				Poll::Ready(None)
			}
			Poll::Pending => Poll::Pending,
		}
	}
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Create an `AcpStream` and its sender channel.
///
/// Returns `(stream, sender)` so the caller can feed chunks into the stream
/// from a notification handler.
///
/// The channel is buffered with a capacity of 256 to absorb bursts from
/// fast-producing agents without blocking the notification dispatcher.
pub fn create_stream(
	timeout_ms: u64,
	permission_active: Arc<AtomicBool>,
	cancellation: CancellationToken,
) -> (AcpStream, mpsc::Sender<StreamChunk>) {
	let (tx, rx) = mpsc::channel(256);
	let stream = AcpStream::new(rx, timeout_ms, permission_active, cancellation);
	(stream, tx)
}

// ---------------------------------------------------------------------------
// Session update parsing
// ---------------------------------------------------------------------------

/// Parse a `session/update` notification into a [`StreamChunk`].
///
/// Examines the notification params to determine the chunk type:
/// - `sessionUpdate == "agent_message_chunk"` with text content → `Delta`
/// - `sessionUpdate == "tool_call"` → `ToolCall`
/// - `sessionUpdate == "tool_call_update"` → `ToolCallUpdate`
/// - Anything else → `None`
///
/// This function does **not** produce `Complete` chunks — those are
/// synthesized by the caller when the `session/prompt` response arrives.
pub fn parse_session_update(value: &serde_json::Value) -> Option<StreamChunk> {
	// The notification params contain `sessionId` and `update`.
	let update_value = value.get("update")?;

	// Try to determine the update type from the `sessionUpdate` field.
	let session_update_type = update_value.get("sessionUpdate")?.as_str()?;

	match session_update_type {
		"agent_message_chunk" => parse_delta(update_value),
		"tool_call" => parse_tool_call(update_value),
		"tool_call_update" => parse_tool_call_update(update_value),
		_ => None,
	}
}

/// Extract text content from an `agent_message_chunk` update.
fn parse_delta(update: &serde_json::Value) -> Option<StreamChunk> {
	let content = update.get("content")?;

	// Content may be a single block or an array of blocks.
	let blocks: Vec<&serde_json::Value> = if content.is_array() {
		content.as_array().unwrap().iter().collect()
	} else {
		vec![content]
	};

	// Extract text from text-type content blocks.
	let mut text = String::new();
	for block in blocks {
		if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
			if block_type == "text" {
				if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
					text.push_str(t);
				}
			}
		} else if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
			// Fallback: block without explicit `type` but has `text` field.
			text.push_str(t);
		}
	}

	if text.is_empty() {
		None
	} else {
		Some(StreamChunk::Delta { text })
	}
}

/// Extract a tool call from a `tool_call` update.
fn parse_tool_call(update: &serde_json::Value) -> Option<StreamChunk> {
	let tool_call_id = update
		.get("toolCallId")
		.and_then(|v| v.as_str())
		.unwrap_or("")
		.to_string();
	let title = update
		.get("title")
		.and_then(|v| v.as_str())
		.unwrap_or("")
		.to_string();

	let kind_str = update.get("kind").and_then(|v| v.as_str()).unwrap_or("other");
	let kind = parse_tool_call_kind(kind_str);

	let status_str = update
		.get("status")
		.and_then(|v| v.as_str())
		.unwrap_or("pending");
	let status = parse_tool_call_status(status_str);

	Some(StreamChunk::ToolCall {
		tool_call: ToolCall {
			tool_call_id,
			title,
			kind,
			status,
		},
	})
}

/// Extract a tool call update from a `tool_call_update` update.
fn parse_tool_call_update(update: &serde_json::Value) -> Option<StreamChunk> {
	let tool_call_id = update
		.get("toolCallId")
		.and_then(|v| v.as_str())
		.unwrap_or("")
		.to_string();

	let status_str = update
		.get("status")
		.and_then(|v| v.as_str())
		.unwrap_or("in_progress");
	let status = parse_tool_call_status(status_str);

	let content = update.get("content").cloned();

	Some(StreamChunk::ToolCallUpdate {
		update: ToolCallUpdate {
			tool_call_id,
			status,
			content,
		},
	})
}

/// Parse a string into a [`ToolCallKind`], defaulting to `Other`.
fn parse_tool_call_kind(s: &str) -> ToolCallKind {
	match s {
		"read" => ToolCallKind::Read,
		"edit" => ToolCallKind::Edit,
		"delete" => ToolCallKind::Delete,
		"move" => ToolCallKind::Move,
		"search" => ToolCallKind::Search,
		"execute" => ToolCallKind::Execute,
		"think" => ToolCallKind::Think,
		"fetch" => ToolCallKind::Fetch,
		_ => ToolCallKind::Other,
	}
}

/// Parse a string into a [`ToolCallStatus`], defaulting to `Pending`.
fn parse_tool_call_status(s: &str) -> ToolCallStatus {
	match s {
		"pending" => ToolCallStatus::Pending,
		"in_progress" => ToolCallStatus::InProgress,
		"completed" => ToolCallStatus::Completed,
		"failed" => ToolCallStatus::Failed,
		"cancelled" => ToolCallStatus::Cancelled,
		_ => ToolCallStatus::Pending,
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use futures::StreamExt;

	#[tokio::test]
	async fn test_delta_chunks_yielded_in_order() {
		let (tx, rx) = mpsc::channel(16);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		let mut stream = AcpStream::new(rx, 60_000, permission_active, cancellation);

		tx.send(StreamChunk::Delta {
			text: "hello ".into(),
		})
		.await
		.unwrap();
		tx.send(StreamChunk::Delta {
			text: "world".into(),
		})
		.await
		.unwrap();

		let chunk1 = stream.next().await.unwrap();
		match chunk1 {
			StreamChunk::Delta { text } => assert_eq!(text, "hello "),
			_ => panic!("expected Delta chunk"),
		}

		let chunk2 = stream.next().await.unwrap();
		match chunk2 {
			StreamChunk::Delta { text } => assert_eq!(text, "world"),
			_ => panic!("expected Delta chunk"),
		}
	}

	#[tokio::test]
	async fn test_complete_chunk_terminates_stream() {
		let (tx, rx) = mpsc::channel(16);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		let mut stream = AcpStream::new(rx, 60_000, permission_active, cancellation);

		let usage = TokenUsage {
			prompt_tokens: 10,
			completion_tokens: 5,
			total_tokens: 15,
		};

		tx.send(StreamChunk::Delta {
			text: "hi".into(),
		})
		.await
		.unwrap();
		tx.send(StreamChunk::Complete {
			usage: Some(usage),
		})
		.await
		.unwrap();

		// Should get the delta.
		let chunk = stream.next().await.unwrap();
		assert!(matches!(chunk, StreamChunk::Delta { .. }));

		// Should get the complete.
		let chunk = stream.next().await.unwrap();
		match &chunk {
			StreamChunk::Complete { usage } => {
				let u = usage.as_ref().unwrap();
				assert_eq!(u.prompt_tokens, 10);
				assert_eq!(u.total_tokens, 15);
			}
			_ => panic!("expected Complete chunk"),
		}

		// Stream should now yield None.
		let next = stream.next().await;
		assert!(next.is_none());
	}

	#[tokio::test]
	async fn test_timeout_fires_after_inactivity() {
		let (tx, rx) = mpsc::channel(16);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		// Very short timeout: 10ms.
		let mut stream = AcpStream::new(rx, 10, permission_active, cancellation);

		// Send a delta, then wait longer than the timeout.
		tx.send(StreamChunk::Delta {
			text: "hi".into(),
		})
		.await
		.unwrap();

		let chunk = stream.next().await.unwrap();
		assert!(matches!(chunk, StreamChunk::Delta { .. }));

		// Wait for the timeout to expire.
		tokio::time::sleep(Duration::from_millis(50)).await;

		// Next poll should yield a synthetic Complete due to timeout.
		let chunk = stream.next().await.unwrap();
		match chunk {
			StreamChunk::Complete { usage } => assert!(usage.is_none()),
			_ => panic!("expected Complete chunk from timeout"),
		}

		// Stream should now be done.
		let next = stream.next().await;
		assert!(next.is_none());

		// Keep sender alive to prevent channel-close from racing with timeout.
		drop(tx);
	}

	#[tokio::test]
	async fn test_permission_suspends_timeout() {
		let (_tx, rx) = mpsc::channel::<StreamChunk>(16);
		let permission_active = Arc::new(AtomicBool::new(true));
		let cancellation = CancellationToken::new();
		// Very short timeout: 10ms.
		let mut stream = AcpStream::new(rx, 10, Arc::clone(&permission_active), cancellation);

		// Wait longer than the timeout.
		tokio::time::sleep(Duration::from_millis(50)).await;

		// With permission active, the stream should NOT time out.
		// We can't poll `next()` here without blocking (channel is empty and
		// permission is active), so we verify the internal state instead.
		assert!(!stream.completed);

		// Now deactivate permission — timeout should fire on next poll.
		permission_active.store(false, Ordering::Relaxed);

		// Use tokio::select! with a small timeout to avoid hanging if
		// Pending is returned (which it shouldn't be after the timeout).
		let result = tokio::time::timeout(Duration::from_millis(200), stream.next()).await;

		match result {
			Ok(Some(StreamChunk::Complete { usage })) => assert!(usage.is_none()),
			other => panic!("expected timeout Complete chunk, got {:?}", other),
		}
	}

	#[tokio::test]
	async fn test_cancellation_stops_stream() {
		let (_tx, rx) = mpsc::channel::<StreamChunk>(16);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		let mut stream = AcpStream::new(rx, 60_000, permission_active, cancellation.clone());

		// Cancel the token.
		cancellation.cancel();

		// Stream should immediately return None.
		let result = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;
		match result {
			Ok(None) => {} // expected
			other => panic!("expected None after cancellation, got {:?}", other),
		}
	}

	#[tokio::test]
	async fn test_channel_close_ends_stream() {
		let (tx, rx) = mpsc::channel(16);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		let mut stream = AcpStream::new(rx, 60_000, permission_active, cancellation);

		tx.send(StreamChunk::Delta {
			text: "data".into(),
		})
		.await
		.unwrap();

		// Drop the sender to close the channel.
		drop(tx);

		// Should get the buffered delta.
		let chunk = stream.next().await.unwrap();
		assert!(matches!(chunk, StreamChunk::Delta { .. }));

		// Should get None because channel is closed.
		let next = stream.next().await;
		assert!(next.is_none());
	}

	// -----------------------------------------------------------------------
	// parse_session_update tests
	// -----------------------------------------------------------------------

	#[test]
	fn test_parse_delta_from_text_block() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "agent_message_chunk",
				"content": [{"type": "text", "text": "hello world"}]
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::Delta { text } => assert_eq!(text, "hello world"),
			_ => panic!("expected Delta"),
		}
	}

	#[test]
	fn test_parse_delta_from_single_block() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "agent_message_chunk",
				"content": {"type": "text", "text": "single block"}
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::Delta { text } => assert_eq!(text, "single block"),
			_ => panic!("expected Delta"),
		}
	}

	#[test]
	fn test_parse_delta_without_type_field() {
		// Some agents send content without explicit type.
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "agent_message_chunk",
				"content": {"text": "no type field"}
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::Delta { text } => assert_eq!(text, "no type field"),
			_ => panic!("expected Delta"),
		}
	}

	#[test]
	fn test_parse_tool_call() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "tool_call",
				"toolCallId": "tc-123",
				"title": "Read file",
				"kind": "read",
				"status": "pending"
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::ToolCall { tool_call } => {
				assert_eq!(tool_call.tool_call_id, "tc-123");
				assert_eq!(tool_call.title, "Read file");
				assert_eq!(tool_call.kind, ToolCallKind::Read);
				assert_eq!(tool_call.status, ToolCallStatus::Pending);
			}
			_ => panic!("expected ToolCall"),
		}
	}

	#[test]
	fn test_parse_tool_call_update() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "tool_call_update",
				"toolCallId": "tc-123",
				"status": "completed",
				"content": {"result": "file contents"}
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::ToolCallUpdate { update } => {
				assert_eq!(update.tool_call_id, "tc-123");
				assert_eq!(update.status, ToolCallStatus::Completed);
				assert!(update.content.is_some());
			}
			_ => panic!("expected ToolCallUpdate"),
		}
	}

	#[test]
	fn test_parse_unknown_update_returns_none() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "some_other_event"
			}
		});

		assert!(parse_session_update(&value).is_none());
	}

	#[test]
	fn test_parse_missing_update_returns_none() {
		let value = serde_json::json!({
			"sessionId": "sess-1"
		});

		assert!(parse_session_update(&value).is_none());
	}

	#[test]
	fn test_parse_empty_text_content_returns_none() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "agent_message_chunk",
				"content": [{"type": "text", "text": ""}]
			}
		});

		assert!(parse_session_update(&value).is_none());
	}

	#[test]
	fn test_parse_tool_call_with_defaults() {
		// Missing kind and status should fall back to defaults.
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "tool_call",
				"toolCallId": "tc-456",
				"title": "Unknown tool"
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::ToolCall { tool_call } => {
				assert_eq!(tool_call.kind, ToolCallKind::Other);
				assert_eq!(tool_call.status, ToolCallStatus::Pending);
			}
			_ => panic!("expected ToolCall"),
		}
	}

	#[test]
	fn test_parse_tool_call_update_with_defaults() {
		let value = serde_json::json!({
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "tool_call_update",
				"toolCallId": "tc-789"
			}
		});

		let chunk = parse_session_update(&value).unwrap();
		match chunk {
			StreamChunk::ToolCallUpdate { update } => {
				assert_eq!(update.tool_call_id, "tc-789");
				assert_eq!(update.status, ToolCallStatus::InProgress);
				assert!(update.content.is_none());
			}
			_ => panic!("expected ToolCallUpdate"),
		}
	}

	// -----------------------------------------------------------------------
	// create_stream factory tests
	// -----------------------------------------------------------------------

	#[tokio::test]
	async fn test_create_stream_factory() {
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		let (mut stream, tx) = create_stream(60_000, permission_active, cancellation);

		tx.send(StreamChunk::Delta {
			text: "from factory".into(),
		})
		.await
		.unwrap();

		let chunk = stream.next().await.unwrap();
		match chunk {
			StreamChunk::Delta { text } => assert_eq!(text, "from factory"),
			_ => panic!("expected Delta"),
		}
	}

	// -----------------------------------------------------------------------
	// Tool call kind and status parsing
	// -----------------------------------------------------------------------

	#[test]
	fn test_parse_all_tool_call_kinds() {
		assert_eq!(parse_tool_call_kind("read"), ToolCallKind::Read);
		assert_eq!(parse_tool_call_kind("edit"), ToolCallKind::Edit);
		assert_eq!(parse_tool_call_kind("delete"), ToolCallKind::Delete);
		assert_eq!(parse_tool_call_kind("move"), ToolCallKind::Move);
		assert_eq!(parse_tool_call_kind("search"), ToolCallKind::Search);
		assert_eq!(parse_tool_call_kind("execute"), ToolCallKind::Execute);
		assert_eq!(parse_tool_call_kind("think"), ToolCallKind::Think);
		assert_eq!(parse_tool_call_kind("fetch"), ToolCallKind::Fetch);
		assert_eq!(parse_tool_call_kind("unknown"), ToolCallKind::Other);
	}

	#[test]
	fn test_parse_all_tool_call_statuses() {
		assert_eq!(parse_tool_call_status("pending"), ToolCallStatus::Pending);
		assert_eq!(
			parse_tool_call_status("in_progress"),
			ToolCallStatus::InProgress
		);
		assert_eq!(
			parse_tool_call_status("completed"),
			ToolCallStatus::Completed
		);
		assert_eq!(parse_tool_call_status("failed"), ToolCallStatus::Failed);
		assert_eq!(
			parse_tool_call_status("cancelled"),
			ToolCallStatus::Cancelled
		);
		assert_eq!(parse_tool_call_status("unknown"), ToolCallStatus::Pending);
	}

	// -----------------------------------------------------------------------
	// Multiple chunk types in sequence
	// -----------------------------------------------------------------------

	#[tokio::test]
	async fn test_mixed_chunk_sequence() {
		let (tx, rx) = mpsc::channel(16);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();
		let mut stream = AcpStream::new(rx, 60_000, permission_active, cancellation);

		tx.send(StreamChunk::Delta {
			text: "analyzing...".into(),
		})
		.await
		.unwrap();
		tx.send(StreamChunk::ToolCall {
			tool_call: ToolCall {
				tool_call_id: "tc-1".into(),
				title: "Read file".into(),
				kind: ToolCallKind::Read,
				status: ToolCallStatus::Pending,
			},
		})
		.await
		.unwrap();
		tx.send(StreamChunk::ToolCallUpdate {
			update: ToolCallUpdate {
				tool_call_id: "tc-1".into(),
				status: ToolCallStatus::Completed,
				content: Some(serde_json::json!({"output": "file data"})),
			},
		})
		.await
		.unwrap();
		tx.send(StreamChunk::Delta {
			text: "done".into(),
		})
		.await
		.unwrap();
		tx.send(StreamChunk::Complete { usage: None })
			.await
			.unwrap();

		// Consume all chunks and verify order.
		let mut chunks = Vec::new();
		while let Some(chunk) = stream.next().await {
			chunks.push(chunk);
		}

		assert_eq!(chunks.len(), 5);
		assert!(matches!(&chunks[0], StreamChunk::Delta { text } if text == "analyzing..."));
		assert!(matches!(&chunks[1], StreamChunk::ToolCall { .. }));
		assert!(matches!(&chunks[2], StreamChunk::ToolCallUpdate { .. }));
		assert!(matches!(&chunks[3], StreamChunk::Delta { text } if text == "done"));
		assert!(matches!(&chunks[4], StreamChunk::Complete { .. }));
	}
}
