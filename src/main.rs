use clap::{Args, Parser, Subcommand};
use quaigh::equiv::check_equivalence_bounded;
use quaigh::io::{read_network_file, read_pattern_file, write_network_file, write_pattern_file};
use quaigh::sim::{generate_random_patterns, simulate_multiple};
use quaigh::stats;
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
    /// Simulate a logic network
    #[clap(alias = "sim")]
    Simulate(SimulateArgs),
    /// Run test pattern generation for a logic network
    #[clap()]
    Atpg(AtpgArgs),
}

#[derive(Args)]
struct EquivArgs {
    /// First network to compare
    file1: PathBuf,
    /// Second network to compare
    file2: PathBuf,

    /// Number of clock cycles considered
    #[arg(short = 'c', long, default_value_t = 1)]
    num_cycles: usize,

    /// Use only sat solver, skipping internal optimizations
    #[arg(long)]
    sat_only: bool,
}

#[derive(Args)]
struct OptArgs {
    /// Network to optimize
    file: PathBuf,

    /// Output file for optimized network
    #[arg(short = 'o', long)]
    output: PathBuf,
}

#[derive(Args)]
struct ShowArgs {
    /// Network to show
    file: PathBuf,
}

#[derive(Args)]
struct SimulateArgs {
    /// Network to simulate
    network: PathBuf,

    /// Input patterns file
    #[arg(short = 'i', long)]
    input: PathBuf,

    /// Output file for output patterns
    #[arg(short = 'o', long)]
    output: PathBuf,
}

#[derive(Args)]
struct AtpgArgs {
    /// Network to write test patterns for
    network: PathBuf,

    /// Output file for test patterns
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Random seed for test pattern generation
    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Attempt to generate sequential patterns instead of combinatorial
    #[arg(short = 'c', long)]
    num_cycles: Option<usize>,

    /// Number of random patterns to generate
    #[arg(short = 'r', long)]
    num_random: Option<usize>,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::CheckEquivalence(EquivArgs {
            file1,
            file2,
            num_cycles,
            sat_only,
        }) => {
            let aig1 = read_network_file(file1);
            let aig2 = read_network_file(file2);
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
            let res = check_equivalence_bounded(&aig1, &aig2, num_cycles, !sat_only);
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
            let mut aig = read_network_file(file);
            aig.sweep();
            aig.dedup();
            write_network_file(output, &aig);
        }
        Commands::Show(ShowArgs { file }) => {
            let mut aig = read_network_file(file);
            println!("Network stats:\n{}\n\n", stats::stats(&aig));

            aig.sweep();
            aig.dedup();
            println!("After deduplication:\n{}", stats::stats(&aig));
        }
        Commands::Simulate(SimulateArgs {
            network,
            input,
            output,
        }) => {
            let aig = read_network_file(network);
            let input_values = read_pattern_file(input);
            let output_values = simulate_multiple(&aig, &input_values);
            write_pattern_file(output, &output_values);
        }
        Commands::Atpg(AtpgArgs {
            network,
            output,
            seed,
            num_random,
            num_cycles,
        }) => {
            let aig = read_network_file(network);
            let nb_timesteps = num_cycles.unwrap_or(1);
            let nb_patterns = num_random.unwrap_or(4 * (aig.nb_inputs() + 1));
            let patterns =
                generate_random_patterns(aig.nb_inputs(), nb_timesteps, nb_patterns, seed);
            write_pattern_file(output, &patterns);
        }
    }
}
