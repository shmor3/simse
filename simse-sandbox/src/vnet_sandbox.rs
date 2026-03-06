#[derive(Debug, Clone)]
pub enum HostRule {
    Exact(String),
    Wildcard(String),
    Cidr { addr: [u8; 4], prefix: u8 },
}

impl HostRule {
    pub fn parse(s: &str) -> Self {
        if s.starts_with("*.") {
            return HostRule::Wildcard(s.to_string());
        }
        if let Some(cidr) = Self::try_parse_cidr(s) {
            return cidr;
        }
        HostRule::Exact(s.to_string())
    }

    fn try_parse_cidr(s: &str) -> Option<Self> {
        let (ip_str, prefix_str) = s.split_once('/')?;
        let prefix: u8 = prefix_str.parse().ok()?;
        if prefix > 32 {
            return None;
        }
        let parts: Vec<&str> = ip_str.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let mut addr = [0u8; 4];
        for (i, part) in parts.iter().enumerate() {
            addr[i] = part.parse().ok()?;
        }
        Some(HostRule::Cidr { addr, prefix })
    }

    fn matches(&self, host: &str) -> bool {
        match self {
            HostRule::Exact(h) => h.eq_ignore_ascii_case(host),
            HostRule::Wildcard(pattern) => {
                // "*.github.com" matches "api.github.com" but not "github.com"
                let suffix = &pattern[1..]; // ".github.com"
                host.len() > suffix.len()
                    && host[host.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
                    && !host[..host.len() - suffix.len()].contains('.')
            }
            HostRule::Cidr { addr, prefix } => {
                let Some(ip) = Self::parse_ipv4(host) else {
                    return false;
                };
                let mask = if *prefix == 0 {
                    0u32
                } else {
                    !0u32 << (32 - prefix)
                };
                let net = u32::from_be_bytes(*addr);
                let target = u32::from_be_bytes(ip);
                (net & mask) == (target & mask)
            }
        }
    }

    fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let mut addr = [0u8; 4];
        for (i, part) in parts.iter().enumerate() {
            addr[i] = part.parse().ok()?;
        }
        Some(addr)
    }
}

#[derive(Debug, Clone)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

pub struct NetSandboxConfig {
    pub allowed_hosts: Vec<HostRule>,
    pub allowed_ports: Vec<PortRange>,
    pub allowed_protocols: Vec<String>,
    pub default_timeout_ms: u64,
    pub max_response_bytes: u64,
    pub max_connections: usize,
}

impl Default for NetSandboxConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            allowed_ports: Vec::new(),
            allowed_protocols: Vec::new(),
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 50,
        }
    }
}

impl NetSandboxConfig {
    pub fn validate(&self, host: &str, port: u16, protocol: &str) -> Result<(), String> {
        // Host check: if empty, block all (safe default)
        if !self.allowed_hosts.iter().any(|rule| rule.matches(host)) {
            return Err(format!("host '{host}' not in allowed hosts"));
        }

        // Port check: if empty, allow all
        if !self.allowed_ports.is_empty()
            && !self
                .allowed_ports
                .iter()
                .any(|r| port >= r.start && port <= r.end)
        {
            return Err(format!("port {port} not in allowed ports"));
        }

        // Protocol check: if empty, allow all
        if !self.allowed_protocols.is_empty()
            && !self
                .allowed_protocols
                .iter()
                .any(|p| p.eq_ignore_ascii_case(protocol))
        {
            return Err(format!("protocol '{protocol}' not allowed"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> NetSandboxConfig {
        NetSandboxConfig::default()
    }

    #[test]
    fn empty_allowlist_blocks_everything() {
        let cfg = default_config();
        let err = cfg.validate("example.com", 80, "http").unwrap_err();
        assert!(err.contains("not in allowed hosts"));
    }

    #[test]
    fn exact_host_match() {
        let mut cfg = default_config();
        cfg.allowed_hosts
            .push(HostRule::Exact("api.example.com".into()));
        assert!(cfg.validate("api.example.com", 80, "http").is_ok());
        assert!(cfg.validate("other.example.com", 80, "http").is_err());
    }

    #[test]
    fn wildcard_host_match() {
        let mut cfg = default_config();
        cfg.allowed_hosts
            .push(HostRule::Wildcard("*.github.com".into()));
        assert!(cfg.validate("api.github.com", 443, "https").is_ok());
        assert!(cfg.validate("raw.github.com", 443, "https").is_ok());
        assert!(cfg.validate("github.com", 443, "https").is_err());
        assert!(cfg.validate("evil.com", 443, "https").is_err());
    }

    #[test]
    fn cidr_host_match() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Cidr {
            addr: [10, 0, 0, 0],
            prefix: 8,
        });
        assert!(cfg.validate("10.0.0.1", 80, "http").is_ok());
        assert!(cfg.validate("10.255.255.255", 80, "http").is_ok());
        assert!(cfg.validate("11.0.0.1", 80, "http").is_err());
    }

    #[test]
    fn port_range_validation() {
        let mut cfg = default_config();
        cfg.allowed_hosts
            .push(HostRule::Exact("example.com".into()));
        cfg.allowed_ports.push(PortRange { start: 80, end: 80 });
        cfg.allowed_ports
            .push(PortRange { start: 443, end: 443 });
        assert!(cfg.validate("example.com", 80, "http").is_ok());
        assert!(cfg.validate("example.com", 443, "https").is_ok());
        assert!(cfg.validate("example.com", 8080, "http").is_err());
    }

    #[test]
    fn empty_ports_allows_all() {
        let mut cfg = default_config();
        cfg.allowed_hosts
            .push(HostRule::Exact("example.com".into()));
        assert!(cfg.validate("example.com", 12345, "http").is_ok());
    }

    #[test]
    fn protocol_restriction() {
        let mut cfg = default_config();
        cfg.allowed_hosts
            .push(HostRule::Exact("example.com".into()));
        cfg.allowed_protocols.push("https".into());
        assert!(cfg.validate("example.com", 443, "https").is_ok());
        assert!(cfg.validate("example.com", 80, "http").is_err());
    }

    #[test]
    fn empty_protocols_allows_all() {
        let mut cfg = default_config();
        cfg.allowed_hosts
            .push(HostRule::Exact("example.com".into()));
        assert!(cfg.validate("example.com", 80, "tcp").is_ok());
    }

    #[test]
    fn parse_host_rule_exact() {
        let rule = HostRule::parse("api.example.com");
        assert!(matches!(rule, HostRule::Exact(h) if h == "api.example.com"));
    }

    #[test]
    fn parse_host_rule_wildcard() {
        let rule = HostRule::parse("*.github.com");
        assert!(matches!(rule, HostRule::Wildcard(p) if p == "*.github.com"));
    }

    #[test]
    fn parse_host_rule_cidr() {
        let rule = HostRule::parse("192.168.1.0/24");
        assert!(matches!(rule, HostRule::Cidr { prefix: 24, .. }));
    }
}
