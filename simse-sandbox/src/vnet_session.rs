use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug)]
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

pub struct SessionManager {
    sessions: HashMap<String, NetSession>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn create(&mut self, session_type: SessionType, target: &str, scheme: Scheme) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        self.sessions.insert(
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
        id
    }

    pub fn get(&self, id: &str) -> Option<&NetSession> {
        self.sessions.get(id)
    }

    pub fn list(&self) -> Vec<&NetSession> {
        self.sessions.values().collect()
    }

    pub fn close(&mut self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    pub fn record_activity(&mut self, id: &str, bytes_sent: u64, bytes_received: u64) {
        if let Some(session) = self.sessions.get_mut(id) {
            session.last_active_at = now_ms();
            session.bytes_sent += bytes_sent;
            session.bytes_received += bytes_received;
        }
    }

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
        let mut mgr = SessionManager::new();
        let id = mgr.create(SessionType::Ws, "example.com:443", Scheme::Mock);
        let session = mgr.get(&id).unwrap();
        assert_eq!(session.target, "example.com:443");
        assert_eq!(session.session_type, SessionType::Ws);
        assert_eq!(session.scheme, Scheme::Mock);
    }

    #[test]
    fn list_sessions() {
        let mut mgr = SessionManager::new();
        mgr.create(SessionType::Ws, "a.com:443", Scheme::Mock);
        mgr.create(SessionType::Tcp, "b.com:80", Scheme::Net);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn close_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.create(SessionType::Tcp, "c.com:22", Scheme::Net);
        assert!(mgr.close(&id));
        assert!(mgr.get(&id).is_none());
        assert!(!mgr.close(&id));
    }

    #[test]
    fn update_activity() {
        let mut mgr = SessionManager::new();
        let id = mgr.create(SessionType::Ws, "d.com:443", Scheme::Mock);
        let before = mgr.get(&id).unwrap().last_active_at;
        mgr.record_activity(&id, 100, 200);
        let after = mgr.get(&id).unwrap();
        assert!(after.last_active_at >= before);
        assert_eq!(after.bytes_sent, 100);
        assert_eq!(after.bytes_received, 200);
    }

    #[test]
    fn active_count() {
        let mut mgr = SessionManager::new();
        assert_eq!(mgr.active_count(), 0);
        let id1 = mgr.create(SessionType::Ws, "a.com:443", Scheme::Mock);
        mgr.create(SessionType::Tcp, "b.com:80", Scheme::Net);
        assert_eq!(mgr.active_count(), 2);
        mgr.close(&id1);
        assert_eq!(mgr.active_count(), 1);
    }
}
