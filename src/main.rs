//! Binary for Quaigh

#![warn(missing_docs)]

mod cmd;

pub mod atpg;
pub mod equiv;
pub mod io;
pub mod network;
pub mod optim;
pub mod sim;

use clap::Parser;
pub use network::{Gate, NaryType, Network, Signal};

#[doc(hidden)]
fn main() {
    let cli = cmd::Cli::parse();

    match cli.command {
        cmd::Commands::CheckEquivalence(a) => a.run(),
        cmd::Commands::Optimize(a) => a.run(),
        cmd::Commands::Show(a) => a.run(),
        cmd::Commands::Simulate(a) => a.run(),
        cmd::Commands::Atpg(a) => a.run(),
        cmd::Commands::Convert(a) => a.run(),
    }
}
