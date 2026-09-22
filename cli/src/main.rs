mod log;

use clap::{Parser, Subcommand};

use doze::{execute, Graph, LocalCache, ProcedureId, ResolveMode, Resolver, TopologicalResolver};
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
            tracing::debug!("run command called");

            let mut graph = Graph::new();
            let resolver = TopologicalResolver {};
            let registry = doze::Registry::with_builtins();
            let copy_proc = ProcedureId("utils:copy".to_string());

            tracing::debug!(
                "added rule checksum: {}",
                graph
                    .add_rule(
                        vec!["utils.c".into()],
                        vec!["zeub.exe".into()],
                        "".into(),
                        "".into(),
                        copy_proc.clone(),
                        &registry
                    )
                    .unwrap()
            );
            tracing::debug!(
                "added rule checksum: {}",
                graph
                    .add_rule(
                        vec!["main.c".into()],
                        vec!["a.out".into()],
                        "".into(),
                        "".into(),
                        copy_proc.clone(),
                        &registry
                    )
                    .unwrap()
            );
            tracing::debug!(
                "added rule checksum: {}",
                graph
                    .add_rule(
                        vec!["myheader.h".into()],
                        vec!["main.c".into()],
                        "".into(),
                        "".into(),
                        copy_proc.clone(),
                        &registry
                    )
                    .unwrap()
            );
            tracing::debug!(
                "added rule checksum: {}",
                graph
                    .add_rule(
                        vec!["root.config.h".into()],
                        vec!["myheader.h".into()],
                        "".into(),
                        "".into(),
                        copy_proc.clone(),
                        &registry
                    )
                    .unwrap()
            );
            tracing::debug!(
                "added rule checksum: {}",
                graph
                    .add_rule(
                        vec!["myheader.h".into()],
                        vec!["utils.c".into()],
                        "".into(),
                        "".into(),
                        copy_proc.clone(),
                        &registry
                    )
                    .unwrap()
            );

            let mut cache = LocalCache::new(".doze").unwrap();
            let plan = resolver
                .resolve(ResolveMode::Full, &graph, &mut cache)
                .unwrap();

            tracing::debug!("{:?}", plan.rules);

            let report = execute(&plan, &mut graph, &registry, &mut cache);

            tracing::debug!("{:?}", report);
        }
    }
}
