use clap::{Args, Parser, Subcommand};
use quaigh::{
    equiv::check_equivalence_bounded,
    io::{parse_file, write_file},
    stats,
};
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
    /// Optimize a logic network
    #[clap(alias = "opt")]
    Optimize(OptArgs),
    /// Show statistics about a logic network
    #[clap()]
    Show(ShowArgs),
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
struct OptArgs {
    file: PathBuf,

    #[arg(short = 'o', long)]
    output: PathBuf,
}

#[derive(Args)]
struct ShowArgs {
    file: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::CheckEquivalence(EquivArgs {
            file1,
            file2,
            num_cycles,
        }) => {
            let aig1 = parse_file(file1);
            let aig2 = parse_file(file2);
            if aig1.nb_inputs() != aig2.nb_inputs() {
                println!(
                    "Different number of inputs: {} vs {}. Networks are not equivalent",
                    aig1.nb_inputs(),
                    aig2.nb_inputs()
                );
                std::process::exit(1);
            }
            if aig1.nb_outputs() != aig2.nb_outputs() {
                println!(
                    "Different number of outputs: {} vs {}. Networks are not equivalent",
                    aig1.nb_outputs(),
                    aig2.nb_outputs()
                );
                std::process::exit(1);
            }
            let res = check_equivalence_bounded(&aig1, &aig2, num_cycles);
            let is_comb = aig1.is_comb() && aig2.is_comb();
            match res {
                Err(err) => {
                    println!("Networks are not equivalent");
                    println!("Test pattern:");
                    // TODO: extract the names here
                    for (i, v) in err.iter().enumerate() {
                        print!("{}: ", i + 1);
                        for b in v {
                            print!("{}", if *b { "0" } else { "1" });
                        }
                    }
                    std::process::exit(1);
                }
                Ok(()) => {
                    if is_comb {
                        println!("Networks are equivalent");
                    } else {
                        println!("Networks are equivalent up to {} cycles", num_cycles);
                    }
                    std::process::exit(0);
                }
            }
        }
        Commands::Optimize(OptArgs { file, output }) => {
            let aig = parse_file(file);
            write_file(output, &aig);
        }
        Commands::Show(ShowArgs { file }) => {
            let mut aig = parse_file(file);
            println!("Network stats:\n{}\n\n", stats::stats(&aig));

            aig.sweep();
            aig.dedup();
            println!("After deduplication:\n{}", stats::stats(&aig));
        }
    }
}
