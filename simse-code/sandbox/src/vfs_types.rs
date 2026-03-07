// ---------------------------------------------------------------------------
// VFS shared data types
//
// Extracted from simse-vfs/src/protocol.rs — only the data-type structs used
// by vfs_store.rs and vfs_disk.rs.  JSON-RPC request/response/param types
// stay in the sandbox's protocol.rs.
// ---------------------------------------------------------------------------

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatResult {
    pub path: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub size: u64,
    pub created_at: u64,
    pub modified_at: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileResult {
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    pub size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub path: String,
    pub line: usize,
    pub column: usize,
    #[serde(rename = "match")]
    pub match_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_before: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_after: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub version: usize,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    pub size: u64,
    pub timestamp: u64,
}
