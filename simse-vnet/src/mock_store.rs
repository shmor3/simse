use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,
    pub delay_ms: Option<u64>,
}

#[derive(Debug)]
struct MockDefinition {
    id: String,
    method: Option<String>,
    url_pattern: String,
    compiled: Regex,
    response: MockResponse,
    times: Option<usize>,
    remaining: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MockHit {
    pub mock_id: String,
    pub url: String,
    pub method: Option<String>,
    pub timestamp: u64,
}

pub struct MockListItem {
    pub id: String,
    pub method: Option<String>,
    pub url_pattern: String,
    pub status: u16,
    pub times: Option<usize>,
    pub remaining: Option<usize>,
}

pub struct MockStore {
    mocks: Vec<MockDefinition>,
    hits: Vec<MockHit>,
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            mocks: Vec::new(),
            hits: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        method: Option<String>,
        url_pattern: &str,
        response: MockResponse,
        times: Option<usize>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let compiled = glob_to_regex(url_pattern);
        self.mocks.push(MockDefinition {
            id: id.clone(),
            method,
            url_pattern: url_pattern.to_string(),
            compiled,
            response,
            times,
            remaining: times,
        });
        id
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let len_before = self.mocks.len();
        self.mocks.retain(|m| m.id != id);
        self.mocks.len() < len_before
    }

    pub fn find_match(
        &mut self,
        url: &str,
        method: Option<&str>,
    ) -> Option<(String, MockResponse)> {
        let idx = self.mocks.iter().position(|m| {
            // Check remaining count
            if let Some(0) = m.remaining {
                return false;
            }
            // Check method (None mock method = match any)
            if let Some(ref mock_method) = m.method {
                if let Some(req_method) = method {
                    if !mock_method.eq_ignore_ascii_case(req_method) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            // Check URL pattern
            m.compiled.is_match(url)
        })?;

        let mock = &mut self.mocks[idx];
        let id = mock.id.clone();
        let response = mock.response.clone();

        // Decrement remaining
        if let Some(ref mut remaining) = mock.remaining {
            *remaining -= 1;
        }

        // Record hit
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.hits.push(MockHit {
            mock_id: id.clone(),
            url: url.to_string(),
            method: method.map(String::from),
            timestamp: now,
        });

        Some((id, response))
    }

    pub fn list(&self) -> Vec<MockListItem> {
        self.mocks
            .iter()
            .filter(|m| m.remaining != Some(0))
            .map(|m| MockListItem {
                id: m.id.clone(),
                method: m.method.clone(),
                url_pattern: m.url_pattern.clone(),
                status: m.response.status,
                times: m.times,
                remaining: m.remaining,
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.mocks.clear();
        self.hits.clear();
    }

    pub fn history(&self) -> &[MockHit] {
        &self.hits
    }
}

/// Convert a glob pattern (with `*`) to a regex.
fn glob_to_regex(pattern: &str) -> Regex {
    let mut re = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '.' | '(' | ')' | '[' | ']' | '{' | '}' | '+' | '?' | '^' | '$' | '|' | '\\' => {
                re.push('\\');
                re.push(ch);
            }
            _ => re.push(ch),
        }
    }
    re.push('$');
    Regex::new(&re).expect("invalid glob pattern")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response() -> MockResponse {
        MockResponse {
            status: 200,
            headers: HashMap::new(),
            body: "{\"ok\":true}".into(),
            body_type: "text".into(),
            delay_ms: None,
        }
    }

    #[test]
    fn register_and_match() {
        let mut store = MockStore::new();
        let id = store.register(
            Some("GET".into()),
            "mock://api.example.com/users",
            make_response(),
            None,
        );
        let hit = store.find_match("mock://api.example.com/users", Some("GET"));
        assert!(hit.is_some());
        let (matched_id, resp) = hit.unwrap();
        assert_eq!(matched_id, id);
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn glob_pattern_matching() {
        let mut store = MockStore::new();
        store.register(None, "mock://api.example.com/*", make_response(), None);
        assert!(store
            .find_match("mock://api.example.com/users", None)
            .is_some());
        assert!(store
            .find_match("mock://api.example.com/posts/123", None)
            .is_some());
        assert!(store.find_match("mock://other.com/users", None).is_none());
    }

    #[test]
    fn method_filtering() {
        let mut store = MockStore::new();
        store.register(
            Some("POST".into()),
            "mock://api.example.com/users",
            make_response(),
            None,
        );
        assert!(store
            .find_match("mock://api.example.com/users", Some("POST"))
            .is_some());
        assert!(store
            .find_match("mock://api.example.com/users", Some("GET"))
            .is_none());
    }

    #[test]
    fn none_method_matches_any() {
        let mut store = MockStore::new();
        store.register(None, "mock://api.example.com/users", make_response(), None);
        assert!(store
            .find_match("mock://api.example.com/users", Some("GET"))
            .is_some());
        assert!(store
            .find_match("mock://api.example.com/users", Some("POST"))
            .is_some());
    }

    #[test]
    fn times_limit_consumes_mock() {
        let mut store = MockStore::new();
        store.register(None, "mock://api.example.com/once", make_response(), Some(1));
        assert!(store
            .find_match("mock://api.example.com/once", None)
            .is_some());
        assert!(store
            .find_match("mock://api.example.com/once", None)
            .is_none());
    }

    #[test]
    fn unregister_removes_mock() {
        let mut store = MockStore::new();
        let id = store.register(None, "mock://x", make_response(), None);
        assert!(store.unregister(&id));
        assert!(store.find_match("mock://x", None).is_none());
    }

    #[test]
    fn clear_removes_all() {
        let mut store = MockStore::new();
        store.register(None, "mock://a", make_response(), None);
        store.register(None, "mock://b", make_response(), None);
        store.clear();
        assert!(store.list().is_empty());
    }

    #[test]
    fn history_tracks_hits() {
        let mut store = MockStore::new();
        store.register(None, "mock://api/*", make_response(), None);
        store.find_match("mock://api/one", Some("GET"));
        store.find_match("mock://api/two", Some("POST"));
        let hist = store.history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].url, "mock://api/one");
        assert_eq!(hist[1].url, "mock://api/two");
    }

    #[test]
    fn first_match_wins() {
        let mut store = MockStore::new();
        let mut resp1 = make_response();
        resp1.status = 200;
        let mut resp2 = make_response();
        resp2.status = 404;
        store.register(None, "mock://api/*", resp1, None);
        store.register(None, "mock://api/*", resp2, None);
        let (_, resp) = store.find_match("mock://api/test", None).unwrap();
        assert_eq!(resp.status, 200);
    }
}
