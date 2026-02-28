use crate::error::VfsError;
use crate::transport::NdjsonTransport;

/// VFS JSON-RPC server — dispatches incoming requests to VFS operations.
pub struct VfsServer {
    transport: NdjsonTransport,
}

impl VfsServer {
    /// Create a new VFS server with the given transport.
    pub fn new(transport: NdjsonTransport) -> Self {
        Self { transport }
    }

    /// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
    pub fn run(&mut self) -> Result<(), VfsError> {
        use std::io::BufRead;

        let stdin = std::io::stdin();
        let reader = stdin.lock();

        for line_result in reader.lines() {
            let line = line_result?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Full dispatch will be implemented in a later task.
            // For now, respond with method-not-found for all requests.
            match serde_json::from_str::<crate::protocol::JsonRpcRequest>(trimmed) {
                Ok(req) => {
                    self.transport.write_error(
                        req.id,
                        crate::protocol::METHOD_NOT_FOUND,
                        format!("Method not found: {}", req.method),
                        None,
                    );
                }
                Err(e) => {
                    tracing::warn!("Parse error: {}", e);
                    self.transport.write_error(
                        0,
                        crate::protocol::INTERNAL_ERROR,
                        "Parse error: invalid JSON",
                        None,
                    );
                }
            }
        }

        Ok(())
    }
}
