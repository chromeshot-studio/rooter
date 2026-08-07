use crate::util::classify::{classify_query, InputKind};
use crate::util::format;
use crate::util::system;
use anyhow::Result;
use inquire::Confirm;
use std::collections::BTreeSet;
use sysinfo::{Pid, System};

pub fn run(target: &str, force: bool) -> Result<()> {
    let sys = system::fresh_system();

    let mut pids: BTreeSet<u32> = BTreeSet::new();
    let source;

    match classify_query(target) {
        InputKind::Number(n) => {
            if system::find_process(&sys, n).is_some() {
                pids.insert(n);
                source = format!("PID {n}");
            } else if n <= 65535 {
                for m in system::sockets_on_port(n as u16) {
                    pids.extend(m.pids);
                }
                source = format!("port {n}");
                if pids.is_empty() {
                    format::info(format!("Nothing found for {n} (no such PID, no socket bound to that port)."));
                    return Ok(());
                }
            } else {
                format::info(format!("No process with PID {n}."));
                return Ok(());
            }
        }
        InputKind::Path(_) | InputKind::Text(_) => {
            let matches = system::processes_matching(&sys, target);
            if matches.is_empty() {
                format::info(format!("No running process matches '{target}'."));
                return Ok(());
            }
            source = format!("processes matching '{target}'");
            pids.extend(matches.iter().map(|(pid, _)| pid.as_u32()));
        }
    }

    format::heading(&format!("Found {} process(es) for {source}", pids.len()));
    for pid in &pids {
        if let Some(p) = system::find_process(&sys, *pid) {
            format::bullet(format!("{} (PID {pid})", p.name().to_string_lossy()));
        }
    }

    if !force {
        let confirmed = Confirm::new(&format!("Kill {} process(es)?", pids.len()))
            .with_default(false)
            .prompt()
            .unwrap_or(false);
        if !confirmed {
            format::info("Cancelled.");
            return Ok(());
        }
    }

    kill_all(&sys, &pids);
    Ok(())
}

fn kill_all(sys: &System, pids: &BTreeSet<u32>) {
    for pid in pids {
        match sys.process(Pid::from_u32(*pid)) {
            Some(p) if p.kill() => format::ok(format!("killed PID {pid}")),
            Some(_) => format::fail(format!("PID {pid}: kill signal failed (permissions?)")),
            None => format::warn(format!("PID {pid}: process disappeared before it could be killed")),
        }
    }
}
