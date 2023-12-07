//! Logic simplification and analysis tools
//!
//! This crate provides tools for logic optimization, synthesis, technology mapping and analysis.
//! Our goal is to provide an easy-to-use library, and improve its quality over time to match industrial tools.
//!
//! # Usage
//!
//! Quaigh features bounded [equivalence checking](https://en.wikipedia.org/wiki/Formal_equivalence_checking),
//! [logic simplification](https://en.wikipedia.org/wiki/Logic_optimization) and
//! [test pattern generation](https://en.wikipedia.org/wiki/Automatic_test_pattern_generation).
//! More features will be added over time, such as technology mapping.
//! At the moment, logic simplification is far from state of the art: for production designs, you should
//! generally stick to the tools included in [Yosys](https://github.com/YosysHQ/yosys).
//!
//! ```bash
//! # Show available commands
//! # At the moment, only .bench files are supported
//! quaigh help
//! # Optimize the logic
//! quaigh opt mydesign.bench -o optimized.bench
//! # Check equivalence between the two
//! quaigh equiv mydesign.bench optimized.bench
//! # Generate test patterns for the optimized design
//! quaigh atpg optimized.bench -o atpg.test
//! ```
//!
//! # Installation
//!
//! Quaigh is written in Rust. It is not published on crates.io yet, but you can install it from the git
//! repository using [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html), Rust's
//! package manager:
//! ```bash
//! cargo install --git https://github.com/Coloquinte/quaigh
//! ```
//!
//! # Development
//!
//! Quaigh main datastructure is a typical Gate-Inverter-Graph to represent a logic circuit.
//! Inverters are implicit, occupying just one bit.
//!
//! To make interoperability and optimization easier, many kinds of logic are supported:
//! * Complex gates such as Xor, Mux and Maj3 are all first class citizens and can coexist in the same circuit;
//! * Flip-flops with enable and reset are represented directly, not as primary inputs and outputs.
//!
//! For more information, browse the documentation locally:
//! ```bash
//! cargo doc --open --no-deps
//! ```

#![warn(missing_docs)]

mod network;

pub mod atpg;
pub mod equiv;
pub mod io;
pub mod sim;

pub use network::{area, generators, stats, Aig, Gate, NaryType, Signal};

use atpg::{expose_dff, generate_random_seq_patterns, generate_test_patterns};
use clap::{Args, Parser, Subcommand};
use equiv::check_equivalence_bounded;
use io::{read_network_file, read_pattern_file, write_network_file, write_pattern_file};
use sim::simulate_multiple;
use std::path::PathBuf;

/// Command line arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Command line arguments
#[derive(Subcommand)]
enum Commands {
    /// Show statistics about a logic network
    ///
    /// Will print statistics on the number of inputs, outputs and gates in the network.
    #[clap()]
    Show(ShowArgs),

    /// Optimize a logic network
    ///
    /// At the moment this is simple constant propagation and deduplication,
    /// but will grow in power over time.
    #[clap(alias = "opt")]
    Optimize(OptArgs),

    /// Simulate a logic network
    ///
    /// This uses the same test pattern format as Atalanta, with one bit per input:
    ///    1: 00011101
    ///    2: 01110000
    #[clap(alias = "sim")]
    Simulate(SimulateArgs),

    /// Test pattern generation for a logic network
    ///
    /// Generate patterns to find all possible faults in a design, assuming
    /// that the primary inputs, outputs and flip-flops can be scanned.
    #[clap()]
    Atpg(AtpgArgs),

    /// Check equivalence between two logic networks
    ///
    /// The command will fail if the two networks are not equivalent, and will output the
    /// failing test pattern.
    #[clap(alias = "equiv")]
    CheckEquivalence(EquivArgs),
}

/// Command arguments for equivalence checking
#[derive(Args)]
struct EquivArgs {
    /// First network to compare
    file1: PathBuf,
    /// Second network to compare
    file2: PathBuf,

    /// Number of clock cycles considered
    #[arg(short = 'c', long, default_value_t = 1)]
    num_cycles: usize,

    /// Use only the Sat solver, skipping internal optimizations
    #[arg(long)]
    sat_only: bool,
}

impl EquivArgs {
    pub fn run(&self) {
        let aig1 = read_network_file(&self.file1);
        let aig2 = read_network_file(&self.file2);
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
        let res = check_equivalence_bounded(&aig1, &aig2, self.num_cycles, !self.sat_only);
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
                    println!("Networks are equivalent up to {} cycles", self.num_cycles);
                }
                std::process::exit(0);
            }
        }
    }
}

/// Command arguments for optimization
#[derive(Args)]
struct OptArgs {
    /// Network to optimize
    file: PathBuf,

    /// Output file for optimized network
    #[arg(short = 'o', long)]
    output: PathBuf,
}

impl OptArgs {
    pub fn run(&self) {
        let mut aig = read_network_file(&self.file);
        aig.sweep();
        aig.dedup();
        write_network_file(&self.output, &aig);
    }
}

/// Command arguments for network informations
#[derive(Args)]
struct ShowArgs {
    /// Network to show
    file: PathBuf,
}

impl ShowArgs {
    pub fn run(&self) {
        let mut aig = read_network_file(&self.file);
        println!("Network stats:\n{}\n\n", stats::stats(&aig));

        aig.sweep();
        aig.dedup();
        println!("After deduplication:\n{}", stats::stats(&aig));
    }
}

/// Command arguments for simulation
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

    /// Expose flip-flops as primary inputs. Used after test pattern generation
    #[arg(long)]
    expose_ff: bool,
}

impl SimulateArgs {
    pub fn run(&self) {
        let mut aig = read_network_file(&self.network);
        if self.expose_ff {
            aig = expose_dff(&aig);
        }
        let input_values = read_pattern_file(&self.input);
        let output_values = simulate_multiple(&aig, &input_values);
        write_pattern_file(&self.output, &output_values);
    }
}

/// Command arguments for test pattern generation
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

    /// Attempt to generate sequential patterns (random only)
    #[arg(short = 'c', long)]
    num_cycles: Option<usize>,

    /// Number of random patterns to generate
    #[arg(short = 'r', long)]
    num_random: Option<usize>,
}

impl AtpgArgs {
    pub fn run(&self) {
        let mut aig = read_network_file(&self.network);

        if self.num_cycles.is_none() && self.num_random.is_none() {
            if !aig.is_comb() {
                println!("Exposing flip-flops for a sequential network");
                aig = expose_dff(&aig);
            }
            let patterns = generate_test_patterns(&aig, self.seed);
            let seq_patterns = patterns.iter().map(|p| vec![p.clone()]).collect();
            write_pattern_file(&self.output, &seq_patterns);
        } else {
            println!("Generating only random patterns for multiple cycles");
            let nb_timesteps = self.num_cycles.unwrap_or(1);
            let nb_patterns = self.num_random.unwrap_or(4 * (aig.nb_inputs() + 1));
            let seq_patterns =
                generate_random_seq_patterns(aig.nb_inputs(), nb_timesteps, nb_patterns, self.seed);
            write_pattern_file(&self.output, &seq_patterns);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::CheckEquivalence(a) => a.run(),
        Commands::Optimize(a) => a.run(),
        Commands::Show(a) => a.run(),
        Commands::Simulate(a) => a.run(),
        Commands::Atpg(a) => a.run(),
    }
}
