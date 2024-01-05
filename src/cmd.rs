//! Command line interface

use crate::atpg::{expose_dff, generate_comb_test_patterns, generate_random_seq_patterns};
use crate::equiv::check_equivalence_bounded;
use crate::io::{read_network_file, read_pattern_file, write_network_file, write_pattern_file};
use crate::optim;
use crate::sim::simulate;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Command line arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Command line arguments
#[derive(Subcommand)]
pub enum Commands {
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
    /// Full fault coverage is achieved using a SAT solver.
    ///
    /// Fault types are:
    ///   * Output stuck-at fault, where the output of the gate is stuck at a constant value
    ///   * Input stuck-at fault, where the input of the gate is stuck at a constant value
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
pub struct EquivArgs {
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
pub struct OptArgs {
    /// Network to optimize
    file: PathBuf,

    /// Output file for optimized network
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Effort level
    #[arg(long, default_value_t = 1)]
    effort: u64,

    /// Seed for randomized algorithms
    #[arg(long)]
    seed: Option<u64>,
}

impl OptArgs {
    pub fn run(&self) {
        let mut aig = read_network_file(&self.file);
        if let Some(s) = self.seed {
            aig.shuffle(s);
        }
        aig.cleanup();
        aig.make_canonical();
        optim::share_logic(&mut aig, 64);
        for _ in 0..self.effort {
            optim::infer_xor_mux(&mut aig);
            optim::infer_dffe(&mut aig);
            optim::share_logic(&mut aig, 64);
        }
        write_network_file(&self.output, &aig);
    }
}

/// Command arguments for network informations
#[derive(Args)]
pub struct ShowArgs {
    /// Network to show
    file: PathBuf,
}

impl ShowArgs {
    pub fn run(&self) {
        use crate::network::stats::stats;
        let aig = read_network_file(&self.file);
        println!("Network stats:\n{}\n\n", stats(&aig));
    }
}

/// Command arguments for simulation
#[derive(Args)]
pub struct SimulateArgs {
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
        let mut output_values = Vec::new();
        for pattern in &input_values {
            output_values.push(simulate(&aig, pattern));
        }
        write_pattern_file(&self.output, &output_values);
    }
}

/// Command arguments for test pattern generation
#[derive(Args)]
pub struct AtpgArgs {
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
            let patterns = generate_comb_test_patterns(&aig, self.seed);
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
