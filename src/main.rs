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
    /// Check equivalence between two logic networks
    #[clap(alias = "equiv")]
    CheckEquivalence(EquivArgs),
    /// Optimize the logic network
    #[clap(alias = "opt")]
    Optimize(OptimizeArgs),
}

#[derive(Args)]
struct EquivArgs {
    file1: PathBuf,
    file2: PathBuf,

    /// Number of clock cycles considered
    #[arg(short = 'c', long, default_value_t = 1)]
    num_cycles: usize,
}

#[derive(Args)]
struct OptimizeArgs {
    file: PathBuf,

    #[arg(short = 'o', long)]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();
}
