use clap::{Parser, Subcommand};

pub mod doze;

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
            println!("run command called");
        }
    }

    println!("Hello, world!");
}
