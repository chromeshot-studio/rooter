mod cli;
mod commands;
mod interactive;
mod util;

use clap::Parser;
use cli::{Cli, Commands};
use commands::{ask, clean, config, doctor, envcheck, gen, kill, net, ports, secrets, serve, ship, stash, where_cmd, why};
use util::format;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        None => interactive::run(),
        Some(Commands::Why { query }) => why::run(&query.join(" ")),
        Some(Commands::Where { query }) => where_cmd::run(&query.join(" ")),
        Some(Commands::Ship { message, yes, no_push, pr, ai }) => ship::run(ship::ShipOptions {
            message,
            yes,
            no_push,
            pr,
            ai,
        }),
        Some(Commands::Envcheck) => envcheck::run(),
        Some(Commands::Doctor) => doctor::run(),
        Some(Commands::Stash { action }) => stash::run(action),
        Some(Commands::Kill { target, force }) => kill::run(&target, force),
        Some(Commands::Ports { filter }) => ports::run(filter),
        Some(Commands::Clean { yes, force }) => clean::run(yes, force),
        Some(Commands::Secrets { path }) => match secrets::run(path) {
            Ok(true) => std::process::exit(0),
            Ok(false) => std::process::exit(1),
            Err(e) => Err(e),
        },
        Some(Commands::Gen { action }) => gen::run(action),
        Some(Commands::Serve { port, dir, open }) => serve::run(port, dir, open),
        Some(Commands::Net { target }) => net::run(target),
        Some(Commands::Ask { query }) => ask::run(&query.join(" ")),
        Some(Commands::Config { url, model }) => config::run(url, model),
    };

    if let Err(e) = result {
        format::error(e);
        std::process::exit(1);
    }
}
