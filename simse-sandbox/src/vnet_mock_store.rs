use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use im::Vector as ImVector;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Clone)]
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

/// Mock store using persistent `im::Vector` for cheap cloning
/// and functional-style owned-return state transitions.
#[derive(Debug, Clone)]
pub struct MockStore {
    mocks: ImVector<MockDefinition>,
    hits: ImVector<MockHit>,
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            mocks: ImVector::new(),
            hits: ImVector::new(),
        }
    }

    /// Register a new mock definition. Owned-return: consumes self, returns `(Self, String)`.
    pub fn register(
        mut self,
        method: Option<String>,
        url_pattern: &str,
        response: MockResponse,
        times: Option<usize>,
    ) -> (Self, String) {
        let id = uuid::Uuid::new_v4().to_string();
        let compiled = glob_to_regex(url_pattern);
        self.mocks.push_back(MockDefinition {
            id: id.clone(),
            method,
            url_pattern: url_pattern.to_string(),
            compiled,
            response,
            times,
            remaining: times,
        });
        (self, id)
    }

    /// Unregister a mock by ID. Owned-return: consumes self, returns `(Self, bool)`.
    pub fn unregister(self, id: &str) -> (Self, bool) {
        let len_before = self.mocks.len();
        let mocks: ImVector<MockDefinition> = self
            .mocks
            .into_iter()
            .filter(|m| m.id != id)
            .collect();
        let removed = mocks.len() < len_before;
        (Self { mocks, hits: self.hits }, removed)
    }

    /// Find a matching mock for the given URL/method, decrement remaining count,
    /// and record a hit. Owned-return: consumes self, returns `(Self, Option<(String, MockResponse)>)`.
    pub fn find_match(
        mut self,
        url: &str,
        method: Option<&str>,
    ) -> (Self, Option<(String, MockResponse)>) {
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
        });

        let Some(idx) = idx else {
            return (self, None);
        };

        let mock = &self.mocks[idx];
        let id = mock.id.clone();
        let response = mock.response.clone();

        // Decrement remaining
        if mock.remaining.is_some() {
            let mut updated = mock.clone();
            if let Some(ref mut remaining) = updated.remaining {
                *remaining -= 1;
            }
            self.mocks.set(idx, updated);
        }

        // Record hit
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.hits.push_back(MockHit {
            mock_id: id.clone(),
            url: url.to_string(),
            method: method.map(String::from),
            timestamp: now,
        });

        (self, Some((id, response)))
    }

    /// List active mocks (those with remaining > 0 or unlimited). Read-only.
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

    /// Clear all mocks and history. Owned-return: consumes self, returns `Self`.
    pub fn clear(self) -> Self {
        Self {
            mocks: ImVector::new(),
            hits: ImVector::new(),
        }
    }

    /// Get hit history. Read-only.
    pub fn history(&self) -> &ImVector<MockHit> {
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
        let store = MockStore::new();
        let (store, id) = store.register(
            Some("GET".into()),
            "mock://api.example.com/users",
            make_response(),
            None,
        );
        let (_, hit) = store.find_match("mock://api.example.com/users", Some("GET"));
        assert!(hit.is_some());
        let (matched_id, resp) = hit.unwrap();
        assert_eq!(matched_id, id);
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn glob_pattern_matching() {
        let store = MockStore::new();
        let (store, _) = store.register(None, "mock://api.example.com/*", make_response(), None);
        let (store, hit) = store.find_match("mock://api.example.com/users", None);
        assert!(hit.is_some());
        let (store, hit) = store.find_match("mock://api.example.com/posts/123", None);
        assert!(hit.is_some());
        let (_, hit) = store.find_match("mock://other.com/users", None);
        assert!(hit.is_none());
    }

    #[test]
    fn method_filtering() {
        let store = MockStore::new();
        let (store, _) = store.register(
            Some("POST".into()),
            "mock://api.example.com/users",
            make_response(),
            None,
        );
        let (store, hit) = store.find_match("mock://api.example.com/users", Some("POST"));
        assert!(hit.is_some());
        let (_, hit) = store.find_match("mock://api.example.com/users", Some("GET"));
        assert!(hit.is_none());
    }

    #[test]
    fn none_method_matches_any() {
        let store = MockStore::new();
        let (store, _) = store.register(None, "mock://api.example.com/users", make_response(), None);
        let (store, hit) = store.find_match("mock://api.example.com/users", Some("GET"));
        assert!(hit.is_some());
        let (_, hit) = store.find_match("mock://api.example.com/users", Some("POST"));
        assert!(hit.is_some());
    }

    #[test]
    fn times_limit_consumes_mock() {
        let store = MockStore::new();
        let (store, _) = store.register(None, "mock://api.example.com/once", make_response(), Some(1));
        let (store, hit) = store.find_match("mock://api.example.com/once", None);
        assert!(hit.is_some());
        let (_, hit) = store.find_match("mock://api.example.com/once", None);
        assert!(hit.is_none());
    }

    #[test]
    fn unregister_removes_mock() {
        let store = MockStore::new();
        let (store, id) = store.register(None, "mock://x", make_response(), None);
        let (store, removed) = store.unregister(&id);
        assert!(removed);
        let (_, hit) = store.find_match("mock://x", None);
        assert!(hit.is_none());
    }

    #[test]
    fn clear_removes_all() {
        let store = MockStore::new();
        let (store, _) = store.register(None, "mock://a", make_response(), None);
        let (store, _) = store.register(None, "mock://b", make_response(), None);
        let store = store.clear();
        assert!(store.list().is_empty());
    }

    #[test]
    fn history_tracks_hits() {
        let store = MockStore::new();
        let (store, _) = store.register(None, "mock://api/*", make_response(), None);
        let (store, _) = store.find_match("mock://api/one", Some("GET"));
        let (store, _) = store.find_match("mock://api/two", Some("POST"));
        let hist = store.history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].url, "mock://api/one");
        assert_eq!(hist[1].url, "mock://api/two");
    }

    #[test]
    fn first_match_wins() {
        let store = MockStore::new();
        let mut resp1 = make_response();
        resp1.status = 200;
        let mut resp2 = make_response();
        resp2.status = 404;
        let (store, _) = store.register(None, "mock://api/*", resp1, None);
        let (store, _) = store.register(None, "mock://api/*", resp2, None);
        let (_, hit) = store.find_match("mock://api/test", None);
        let (_, resp) = hit.unwrap();
        assert_eq!(resp.status, 200);
    }
}
