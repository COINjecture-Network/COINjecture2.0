// Address filtering utilities for libp2p
// Prevents poisoning of peerstore with non-routable addresses

use libp2p::Multiaddr;
use std::net::Ipv4Addr;

/// Expected listen port for this network (used to filter ephemeral ports)
pub const EXPECTED_LISTEN_PORT: u16 = 30333;

/// Check if an IPv4 address is link-local (169.254.0.0/16)
fn is_link_local(ip: &Ipv4Addr) -> bool {
    ip.octets()[0] == 169 && ip.octets()[1] == 254
}

/// Check if an IPv4 address is loopback (127.0.0.0/8)
fn is_loopback(ip: &Ipv4Addr) -> bool {
    ip.octets()[0] == 127
}

/// Check if an IPv4 address is unspecified (0.0.0.0/8)
fn is_unspecified(ip: &Ipv4Addr) -> bool {
    ip.octets()[0] == 0
}

/// Check if an IPv4 address is in private RFC1918 ranges
fn is_private_rfc1918(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();
    // 10.0.0.0/8
    if octets[0] == 10 {
        return true;
    }
    // 172.16.0.0/12 (172.16.x.x - 172.31.x.x)
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }
    // 192.168.0.0/16
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }
    false
}

/// Check if an IPv4 address is in Docker's default bridge range (172.17.0.0/16)
fn is_docker_bridge(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 172 && octets[1] == 17
}

/// Check if an IPv4 address is multicast (224.0.0.0/4)
fn is_multicast(ip: &Ipv4Addr) -> bool {
    (224..=239).contains(&ip.octets()[0])
}

/// Check if an IPv4 address is broadcast (255.255.255.255)
fn is_broadcast(ip: &Ipv4Addr) -> bool {
    ip.octets() == [255, 255, 255, 255]
}

/// Check if a port is ephemeral (typically 32768-65535 on Linux, 49152-65535 on Windows)
/// We consider anything above 32767 as potentially ephemeral
fn is_ephemeral_port(port: u16) -> bool {
    port > 32767
}

/// Address filter result with reason for rejection
#[derive(Debug, Clone)]
pub enum AddressFilterResult {
    Accept,
    RejectLinkLocal,
    RejectLoopback,
    RejectUnspecified,
    RejectPrivate,
    RejectDockerBridge,
    RejectMulticast,
    RejectBroadcast,
    RejectEphemeralPort(u16),
    RejectNoTcpPort,
    RejectIpv6,  // For now, we focus on IPv4 in cloud deployments
}

impl AddressFilterResult {
    pub fn is_accepted(&self) -> bool {
        matches!(self, AddressFilterResult::Accept)
    }

    pub fn reason(&self) -> &'static str {
        match self {
            AddressFilterResult::Accept => "accepted",
            AddressFilterResult::RejectLinkLocal => "link-local (169.254.x.x)",
            AddressFilterResult::RejectLoopback => "loopback (127.x.x.x)",
            AddressFilterResult::RejectUnspecified => "unspecified (0.x.x.x)",
            AddressFilterResult::RejectPrivate => "private RFC1918",
            AddressFilterResult::RejectDockerBridge => "docker bridge (172.17.x.x)",
            AddressFilterResult::RejectMulticast => "multicast",
            AddressFilterResult::RejectBroadcast => "broadcast",
            AddressFilterResult::RejectEphemeralPort(_) => "ephemeral port",
            AddressFilterResult::RejectNoTcpPort => "no TCP port",
            AddressFilterResult::RejectIpv6 => "IPv6 (not supported in cloud mode)",
        }
    }
}

/// Filter configuration for address validation
#[derive(Debug, Clone)]
pub struct AddressFilterConfig {
    /// Allow private RFC1918 addresses (useful for local testing)
    pub allow_private: bool,
    /// Allow Docker bridge addresses
    pub allow_docker: bool,
    /// Allow IPv6 addresses
    pub allow_ipv6: bool,
    /// Expected listen port (if set, reject addresses with different ports)
    pub expected_port: Option<u16>,
    /// Reject ephemeral ports even if expected_port is not set
    pub reject_ephemeral_ports: bool,
}

impl Default for AddressFilterConfig {
    fn default() -> Self {
        AddressFilterConfig {
            allow_private: false,  // Cloud deployment: reject private
            allow_docker: false,   // Cloud deployment: reject docker
            allow_ipv6: false,     // Focus on IPv4 for now
            expected_port: Some(EXPECTED_LISTEN_PORT),
            reject_ephemeral_ports: true,
        }
    }
}

impl AddressFilterConfig {
    /// Create a permissive config for local development
    pub fn local_dev() -> Self {
        AddressFilterConfig {
            allow_private: true,
            allow_docker: true,
            allow_ipv6: true,
            expected_port: None,
            reject_ephemeral_ports: false,
        }
    }
}

/// Validate a multiaddr against the filter policy
pub fn validate_multiaddr(addr: &Multiaddr, config: &AddressFilterConfig) -> AddressFilterResult {
    let mut ipv4_addr: Option<Ipv4Addr> = None;
    let mut tcp_port: Option<u16> = None;
    let mut has_ipv6 = false;

    for proto in addr.iter() {
        match proto {
            libp2p::multiaddr::Protocol::Ip4(ip) => {
                ipv4_addr = Some(ip);
            }
            libp2p::multiaddr::Protocol::Ip6(_) => {
                has_ipv6 = true;
            }
            libp2p::multiaddr::Protocol::Tcp(port) => {
                tcp_port = Some(port);
            }
            _ => {}
        }
    }

    // Handle IPv6
    if has_ipv6 && !config.allow_ipv6 {
        return AddressFilterResult::RejectIpv6;
    }

    // If we have an IPv4 address, validate it
    if let Some(ip) = ipv4_addr {
        // Always reject these regardless of config
        if is_link_local(&ip) {
            return AddressFilterResult::RejectLinkLocal;
        }
        if is_loopback(&ip) {
            return AddressFilterResult::RejectLoopback;
        }
        if is_unspecified(&ip) {
            return AddressFilterResult::RejectUnspecified;
        }
        if is_multicast(&ip) {
            return AddressFilterResult::RejectMulticast;
        }
        if is_broadcast(&ip) {
            return AddressFilterResult::RejectBroadcast;
        }

        // Configurable rejections
        if !config.allow_docker && is_docker_bridge(&ip) {
            return AddressFilterResult::RejectDockerBridge;
        }
        if !config.allow_private && is_private_rfc1918(&ip) {
            return AddressFilterResult::RejectPrivate;
        }
    }

    // Validate TCP port
    if let Some(port) = tcp_port {
        // If we have an expected port, reject anything else
        if let Some(expected) = config.expected_port {
            if port != expected && is_ephemeral_port(port) {
                return AddressFilterResult::RejectEphemeralPort(port);
            }
        } else if config.reject_ephemeral_ports && is_ephemeral_port(port) {
            return AddressFilterResult::RejectEphemeralPort(port);
        }
    } else if ipv4_addr.is_some() {
        // We have an IP but no TCP port - this is suspicious for a dialable address
        return AddressFilterResult::RejectNoTcpPort;
    }

    AddressFilterResult::Accept
}

/// Filter a list of multiaddrs, returning only valid ones
pub fn filter_multiaddrs(addrs: Vec<Multiaddr>, config: &AddressFilterConfig) -> Vec<Multiaddr> {
    addrs
        .into_iter()
        .filter(|addr| validate_multiaddr(addr, config).is_accepted())
        .collect()
}

/// Filter and log rejected addresses (useful for debugging)
pub fn filter_multiaddrs_with_logging(
    addrs: Vec<Multiaddr>,
    config: &AddressFilterConfig,
    source: &str,
) -> Vec<Multiaddr> {
    let mut accepted = Vec::new();

    for addr in addrs {
        let result = validate_multiaddr(&addr, config);
        if result.is_accepted() {
            accepted.push(addr);
        } else {
            println!(
                "[ADDR_FILTER] Rejected {} from {}: {}",
                addr,
                source,
                result.reason()
            );
        }
    }

    accepted
}

/// Extract the IPv4 address and port from a multiaddr if present
pub fn extract_ip4_and_port(addr: &Multiaddr) -> Option<(Ipv4Addr, u16)> {
    let mut ip: Option<Ipv4Addr> = None;
    let mut port: Option<u16> = None;

    for proto in addr.iter() {
        match proto {
            libp2p::multiaddr::Protocol::Ip4(i) => ip = Some(i),
            libp2p::multiaddr::Protocol::Tcp(p) => port = Some(p),
            _ => {}
        }
    }

    match (ip, port) {
        (Some(i), Some(p)) => Some((i, p)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_local_rejected() {
        let addr: Multiaddr = "/ip4/169.254.1.1/tcp/30333".parse().unwrap();
        let config = AddressFilterConfig::default();
        assert!(matches!(
            validate_multiaddr(&addr, &config),
            AddressFilterResult::RejectLinkLocal
        ));
    }

    #[test]
    fn test_private_rejected_in_cloud_mode() {
        let addr: Multiaddr = "/ip4/192.168.1.1/tcp/30333".parse().unwrap();
        let config = AddressFilterConfig::default();
        assert!(matches!(
            validate_multiaddr(&addr, &config),
            AddressFilterResult::RejectPrivate
        ));
    }

    #[test]
    fn test_private_allowed_in_local_mode() {
        let addr: Multiaddr = "/ip4/192.168.1.1/tcp/30333".parse().unwrap();
        let config = AddressFilterConfig::local_dev();
        assert!(validate_multiaddr(&addr, &config).is_accepted());
    }

    #[test]
    fn test_ephemeral_port_rejected() {
        let addr: Multiaddr = "/ip4/1.2.3.4/tcp/54321".parse().unwrap();
        let config = AddressFilterConfig::default();
        assert!(matches!(
            validate_multiaddr(&addr, &config),
            AddressFilterResult::RejectEphemeralPort(54321)
        ));
    }

    #[test]
    fn test_public_address_accepted() {
        let addr: Multiaddr = "/ip4/143.110.139.166/tcp/30333".parse().unwrap();
        let config = AddressFilterConfig::default();
        assert!(validate_multiaddr(&addr, &config).is_accepted());
    }

    #[test]
    fn test_docker_bridge_rejected() {
        let addr: Multiaddr = "/ip4/172.17.0.2/tcp/30333".parse().unwrap();
        let config = AddressFilterConfig::default();
        assert!(matches!(
            validate_multiaddr(&addr, &config),
            AddressFilterResult::RejectDockerBridge
        ));
    }
}

