use std::time::{SystemTime, UNIX_EPOCH};

use im::HashMap as ImHashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    Ws,
    Tcp,
}

impl SessionType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ws => "ws",
            Self::Tcp => "tcp",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scheme {
    Mock,
    Net,
}

impl Scheme {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Mock => "mock",
            Self::Net => "net",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetSession {
    pub id: String,
    pub session_type: SessionType,
    pub target: String,
    pub scheme: Scheme,
    pub created_at: u64,
    pub last_active_at: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Session manager using persistent `im::HashMap` for cheap cloning
/// and functional-style owned-return state transitions.
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: ImHashMap<String, NetSession>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: ImHashMap::new(),
        }
    }

    /// Create a new session. Owned-return: consumes self, returns `(Self, String)`.
    pub fn create(mut self, session_type: SessionType, target: &str, scheme: Scheme) -> (Self, String) {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        self.sessions = self.sessions.update(
            id.clone(),
            NetSession {
                id: id.clone(),
                session_type,
                target: target.to_string(),
                scheme,
                created_at: now,
                last_active_at: now,
                bytes_sent: 0,
                bytes_received: 0,
            },
        );
        (self, id)
    }

    /// Get a session by ID. Read-only.
    pub fn get(&self, id: &str) -> Option<&NetSession> {
        self.sessions.get(id)
    }

    /// List all sessions. Read-only.
    pub fn list(&self) -> Vec<&NetSession> {
        self.sessions.values().collect()
    }

    /// Close (remove) a session. Owned-return: consumes self, returns `(Self, bool)`.
    pub fn close(self, id: &str) -> (Self, bool) {
        let had = self.sessions.contains_key(id);
        let sessions = self.sessions.without(id);
        (Self { sessions, ..self }, had)
    }

    /// Record bytes sent/received on an existing session.
    /// Owned-return: consumes self, returns `Self`.
    pub fn record_activity(mut self, id: &str, bytes_sent: u64, bytes_received: u64) -> Self {
        if let Some(session) = self.sessions.get(id) {
            let updated = NetSession {
                last_active_at: now_ms(),
                bytes_sent: session.bytes_sent + bytes_sent,
                bytes_received: session.bytes_received + bytes_received,
                ..session.clone()
            };
            self.sessions = self.sessions.update(id.to_string(), updated);
        }
        self
    }

    /// Number of active sessions. Read-only.
    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_session() {
        let mgr = SessionManager::new();
        let (mgr, id) = mgr.create(SessionType::Ws, "example.com:443", Scheme::Mock);
        let session = mgr.get(&id).unwrap();
        assert_eq!(session.target, "example.com:443");
        assert_eq!(session.session_type, SessionType::Ws);
        assert_eq!(session.scheme, Scheme::Mock);
    }

    #[test]
    fn list_sessions() {
        let mgr = SessionManager::new();
        let (mgr, _) = mgr.create(SessionType::Ws, "a.com:443", Scheme::Mock);
        let (mgr, _) = mgr.create(SessionType::Tcp, "b.com:80", Scheme::Net);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn close_session() {
        let mgr = SessionManager::new();
        let (mgr, id) = mgr.create(SessionType::Tcp, "c.com:22", Scheme::Net);
        let (mgr, removed) = mgr.close(&id);
        assert!(removed);
        assert!(mgr.get(&id).is_none());
        let (_, removed) = mgr.close(&id);
        assert!(!removed);
    }

    #[test]
    fn update_activity() {
        let mgr = SessionManager::new();
        let (mgr, id) = mgr.create(SessionType::Ws, "d.com:443", Scheme::Mock);
        let before = mgr.get(&id).unwrap().last_active_at;
        let mgr = mgr.record_activity(&id, 100, 200);
        let after = mgr.get(&id).unwrap();
        assert!(after.last_active_at >= before);
        assert_eq!(after.bytes_sent, 100);
        assert_eq!(after.bytes_received, 200);
    }

    #[test]
    fn active_count() {
        let mgr = SessionManager::new();
        assert_eq!(mgr.active_count(), 0);
        let (mgr, id1) = mgr.create(SessionType::Ws, "a.com:443", Scheme::Mock);
        let (mgr, _) = mgr.create(SessionType::Tcp, "b.com:80", Scheme::Net);
        assert_eq!(mgr.active_count(), 2);
        let (mgr, _) = mgr.close(&id1);
        assert_eq!(mgr.active_count(), 1);
    }
}
