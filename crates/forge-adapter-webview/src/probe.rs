use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

fn default_port(scheme: &str) -> u16 {
    if scheme.eq_ignore_ascii_case("https") {
        443
    } else {
        80
    }
}

/// For an address on this computer, tries to connect and says why it cannot. Other hosts are not
/// probed: behind a company proxy a direct connect fails while the page would load.
pub fn loopback_unreachable(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, tail) = inner.split_once(']')?;
        (host, tail.strip_prefix(':').and_then(|port| port.parse().ok()).unwrap_or_else(|| default_port(scheme)))
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host, port.parse().ok()?)
    } else {
        (authority, default_port(scheme))
    };
    if !(host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1") {
        return None;
    }
    let addresses: Vec<SocketAddr> = (host, port).to_socket_addrs().ok()?.collect();
    let mut last = String::from("no address");
    for address in addresses {
        match TcpStream::connect_timeout(&address, Duration::from_millis(1500)) {
            Ok(_) => return None,
            Err(error) => last = error.to_string(),
        }
    }
    Some(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn a_listening_local_port_is_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(loopback_unreachable(&format!("http://localhost:{port}/x")), None);
        assert_eq!(loopback_unreachable(&format!("http://127.0.0.1:{port}")), None);
    }

    #[test]
    fn a_closed_local_port_is_reported() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        assert!(loopback_unreachable(&format!("http://127.0.0.1:{port}")).is_some());
    }

    #[test]
    fn other_hosts_and_other_schemes_are_never_probed() {
        assert_eq!(loopback_unreachable("https://example.invalid"), None);
        assert_eq!(loopback_unreachable("file:///tmp/a.html"), None);
    }

    #[test]
    fn the_default_port_follows_the_scheme() {
        assert_eq!(default_port("http"), 80);
        assert_eq!(default_port("https"), 443);
    }
}
