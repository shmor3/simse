use crate::transport::NdjsonTransport;

pub struct VnetServer {
    #[allow(dead_code)]
    transport: NdjsonTransport,
}

impl VnetServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self { transport }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
