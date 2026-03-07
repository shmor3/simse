use std::collections::HashMap;

use serde::Serialize;

/// HTTP response returned by mock or real network requests.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseResult {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,
    pub duration_ms: u64,
    pub bytes_received: u64,
}

/// Aggregate network metrics.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResult {
    pub total_requests: u64,
    pub total_errors: u64,
    pub active_sessions: usize,
    pub bytes_total: u64,
}

/// Summary of a registered mock definition.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockDefinitionInfo {
    pub id: String,
    pub method: Option<String>,
    pub url_pattern: String,
    pub status: u16,
    pub times: Option<usize>,
    pub remaining: Option<usize>,
}

/// Record of a mock being matched / hit.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockHitInfo {
    pub mock_id: String,
    pub url: String,
    pub method: Option<String>,
    pub timestamp: u64,
}

/// Summary of a network session.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub session_type: String,
    pub target: String,
    pub scheme: String,
    pub created_at: u64,
    pub last_active_at: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
