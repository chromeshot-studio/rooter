use crate::util::format;
use crate::util::system;
use anyhow::Result;

pub fn run(filter: Option<String>) -> Result<()> {
    let sys = system::fresh_system();
    let needle = filter.map(|f| f.to_lowercase());

    let rows: Vec<_> = system::port_rows(&sys)
        .into_iter()
        .filter(|row| match &needle {
            None => true,
            Some(n) => {
                row.port.to_string().contains(n.as_str())
                    || row
                        .process_name
                        .as_ref()
                        .map(|p| p.to_lowercase().contains(n.as_str()))
                        .unwrap_or(false)
            }
        })
        .collect();

    if rows.is_empty() {
        format::info("No matching sockets found.");
        return Ok(());
    }

    format::heading(&format!("{} socket(s)", rows.len()));
    format::row("PORT", format!("{:<6} {:<9} {}", "PROTO", "STATE", "OWNER"));
    for row in rows {
        let owner = match (row.pid, &row.process_name) {
            (Some(pid), Some(name)) => format!("{name} ({pid})"),
            (Some(pid), None) => pid.to_string(),
            (None, _) => "-".to_string(),
        };
        format::row(
            &row.port.to_string(),
            format!("{:<6} {:<9} {}", row.protocol, row.state, owner),
        );
    }
    format::info(format!("\n  tip: `rooter kill <port>` to free one up"));

    Ok(())
}
