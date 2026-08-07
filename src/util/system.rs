use netstat2::{
    get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, SocketInfo,
};
use sysinfo::{Pid, Process, System};

pub struct PortMatch {
    pub protocol: &'static str,
    pub local_port: u16,
    pub pids: Vec<u32>,
    pub state: String,
}

/// Every socket currently bound to `port` (TCP or UDP), with owning PIDs.
pub fn sockets_on_port(port: u16) -> Vec<PortMatch> {
    let af = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto = ProtocolFlags::TCP | ProtocolFlags::UDP;
    let sockets: Vec<SocketInfo> = get_sockets_info(af, proto).unwrap_or_default();

    sockets
        .into_iter()
        .filter_map(|s| match &s.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp) if tcp.local_port == port => Some(PortMatch {
                protocol: "TCP",
                local_port: tcp.local_port,
                pids: s.associated_pids.clone(),
                state: format!("{:?}", tcp.state),
            }),
            ProtocolSocketInfo::Udp(udp) if udp.local_port == port => Some(PortMatch {
                protocol: "UDP",
                local_port: udp.local_port,
                pids: s.associated_pids.clone(),
                state: "-".to_string(),
            }),
            _ => None,
        })
        .collect()
}

/// All listening/bound sockets, for "what's using what" style reports.
pub fn all_sockets() -> Vec<PortMatch> {
    let af = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto = ProtocolFlags::TCP | ProtocolFlags::UDP;
    let sockets: Vec<SocketInfo> = get_sockets_info(af, proto).unwrap_or_default();

    sockets
        .into_iter()
        .filter_map(|s| match &s.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp) => Some(PortMatch {
                protocol: "TCP",
                local_port: tcp.local_port,
                pids: s.associated_pids.clone(),
                state: format!("{:?}", tcp.state),
            }),
            ProtocolSocketInfo::Udp(udp) => Some(PortMatch {
                protocol: "UDP",
                local_port: udp.local_port,
                pids: s.associated_pids.clone(),
                state: "-".to_string(),
            }),
        })
        .collect()
}

pub struct PortRow {
    pub port: u16,
    pub protocol: &'static str,
    pub state: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

/// A flattened, deduped, sorted view of every bound socket with its owning
/// process name resolved, for `rooter ports` and `why "ports"`.
pub fn port_rows(sys: &System) -> Vec<PortRow> {
    let mut rows = Vec::new();
    for m in all_sockets() {
        if m.pids.is_empty() {
            rows.push(PortRow {
                port: m.local_port,
                protocol: m.protocol,
                state: m.state.clone(),
                pid: None,
                process_name: None,
            });
            continue;
        }
        for pid in &m.pids {
            rows.push(PortRow {
                port: m.local_port,
                protocol: m.protocol,
                state: m.state.clone(),
                pid: Some(*pid),
                process_name: find_process(sys, *pid).map(|p| p.name().to_string_lossy().to_string()),
            });
        }
    }

    rows.sort_by(|a, b| a.port.cmp(&b.port).then(a.protocol.cmp(b.protocol)));
    rows.dedup_by(|a, b| a.port == b.port && a.protocol == b.protocol && a.pid == b.pid);
    rows
}

pub fn fresh_system() -> System {
    let mut sys = System::new_all();
    sys.refresh_all();
    sys
}

pub fn find_process<'a>(sys: &'a System, pid: u32) -> Option<&'a Process> {
    sys.process(Pid::from_u32(pid))
}

pub fn processes_matching<'a>(sys: &'a System, needle: &str) -> Vec<(&'a Pid, &'a Process)> {
    let needle = needle.to_lowercase();
    let mut matches: Vec<(&Pid, &Process)> = sys
        .processes()
        .iter()
        .filter(|(_, p)| p.name().to_string_lossy().to_lowercase().contains(&needle))
        .collect();
    matches.sort_by_key(|(pid, _)| pid.as_u32());
    matches
}

pub fn top_by_cpu(sys: &System, n: usize) -> Vec<(&Pid, &Process)> {
    let mut all: Vec<(&Pid, &Process)> = sys.processes().iter().collect();
    all.sort_by(|a, b| b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap());
    all.into_iter().take(n).collect()
}

pub fn top_by_memory(sys: &System, n: usize) -> Vec<(&Pid, &Process)> {
    let mut all: Vec<(&Pid, &Process)> = sys.processes().iter().collect();
    all.sort_by(|a, b| b.1.memory().cmp(&a.1.memory()));
    all.into_iter().take(n).collect()
}

/// Sizes of the immediate children of `root` (files counted directly, directories
/// walked recursively but capped so a huge tree can't hang the command), sorted
/// largest first.
pub fn largest_subdirs(root: &std::path::Path, top_n: usize) -> Vec<(std::path::PathBuf, u64)> {
    use std::fs;

    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut sized: Vec<(std::path::PathBuf, u64)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            let bytes = dir_size_capped(&path, 100_000);
            sized.push((path, bytes));
        } else {
            sized.push((path, meta.len()));
        }
    }

    sized.sort_by(|a, b| b.1.cmp(&a.1));
    sized.truncate(top_n);
    sized
}

pub fn dir_size_capped(root: &std::path::Path, max_entries: usize) -> u64 {
    let mut total = 0u64;
    let walker = walkdir::WalkDir::new(root).into_iter();
    for (count, entry) in walker.filter_map(|e| e.ok()).enumerate() {
        if count >= max_entries {
            break;
        }
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}
