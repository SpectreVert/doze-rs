mod log;

use std::path::Path;

use clap::{Parser, Subcommand};

use doze::{execute, LocalCache, ResolveMode, Resolver, Registry, TopologicalResolver};
use dozefile::yaml::{from_path, build_graph};
use doze_procedures_utils as _;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Run,
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Command::Run => {
            let _guard = log::init();
            tracing::debug!("cmd [run]");

            let resolver = TopologicalResolver {};
            let registry = Registry::with_builtins();

            let dozefile = from_path(Path::new("Dozefile.yaml")).unwrap();
            let mut graph = build_graph(dozefile, &registry).unwrap();

            let mut cache = LocalCache::new(".doze").unwrap();
            let plan = resolver
                .resolve(ResolveMode::Terse, &graph, &mut cache)
                .unwrap();

            let _report = execute(&plan, &mut graph, &registry, &mut cache);
        }
    }
}
