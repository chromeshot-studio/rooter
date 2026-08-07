use crate::util::format;
use anyhow::Result;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

pub fn run(target: Option<String>) -> Result<()> {
    match target {
        None => show_local_ip(),
        Some(t) => check_host(&t),
    }
}

fn show_local_ip() -> Result<()> {
    format::heading("Local network");
    match local_outbound_ip() {
        Some(ip) => format::row("Outbound IP", ip),
        None => format::warn("couldn't determine a local outbound IP (no network?)"),
    }
    format::info("");
    format::info("  tip: `rooter net <host>` or `rooter net <host>:<port>` to check reachability");
    Ok(())
}

fn local_outbound_ip() -> Option<std::net::IpAddr> {
    // Doesn't actually send anything - UDP "connect" just picks the local
    // interface/address the OS would use to reach that address.
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

fn check_host(target: &str) -> Result<()> {
    let (host, explicit_port) = match target.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, Some(p.parse::<u16>().unwrap())),
        _ => (target, None),
    };

    format::heading(&format!("Checking {host}"));

    let candidate_ports: Vec<u16> = match explicit_port {
        Some(p) => vec![p],
        None => vec![443, 80],
    };

    let mut resolved: Vec<SocketAddr> = Vec::new();
    for port in &candidate_ports {
        if let Ok(addrs) = (host, *port).to_socket_addrs() {
            resolved.extend(addrs);
        }
    }
    resolved.sort_by_key(|a| a.ip());
    resolved.dedup_by_key(|a| a.ip());

    if resolved.is_empty() {
        format::fail("DNS resolution failed - no such host, or no network");
        return Ok(());
    }

    format::row("Resolves to", resolved.iter().map(|a| a.ip().to_string()).collect::<Vec<_>>().join(", "));

    for port in candidate_ports {
        let mut addrs = match (host, port).to_socket_addrs() {
            Ok(a) => a,
            Err(_) => continue,
        };
        let Some(addr) = addrs.next() else { continue };

        let start = Instant::now();
        match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
            Ok(_) => format::ok(format!("{addr} reachable ({} ms)", start.elapsed().as_millis())),
            Err(e) => format::fail(format!("{addr} unreachable: {e}")),
        }
    }

    Ok(())
}
