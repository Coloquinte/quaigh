use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Equiv(EquivArgs),
}

#[derive(Args)]
struct EquivArgs {
    file1: PathBuf,
    file2: PathBuf,
    #[arg(short, long, default_value_t = 1)]
    num_cycles: usize,
}

fn main() {
    let cli = Cli::parse();
}
