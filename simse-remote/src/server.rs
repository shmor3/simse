use crate::error::RemoteError;
use crate::transport::NdjsonTransport;

pub struct RemoteServer {
    _transport: NdjsonTransport,
}

impl RemoteServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self {
            _transport: transport,
        }
    }

    pub async fn run(&mut self) -> Result<(), RemoteError> {
        todo!("server.run() not yet implemented")
    }
}
