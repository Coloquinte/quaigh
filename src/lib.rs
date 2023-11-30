//! Implementation of And-Inverter-Graph for logic simplification (AIG).
//!
//! This crate provides a flexible representation for logic graphs based on a network of logic gates with implicit inverters.
//! It provides utilities to manipulate and simplify logic functions, check for equivalence, ...
//!
//! It is inspired by the logic synthesis tools [ABC](https://people.eecs.berkeley.edu/~alanmi/abc/) and [Mockturtle](https://mockturtle.readthedocs.io/en/latest/).
//! Our goal is to provide an easy-to-use library for logical synthesis and technology mapping, and improve its quality over time to match industrial tools.
//!
//! # Design
//!
//! Quaigh main representation uses a typical Gate-Inverter-Graph to represent a logic circuit.
//!
//! To make optimization easier, it differs from most similar representations:
//! * Complex gates such as Xor, Mux and Maj3 are all first class citizens and can coexist in the same logic circuit;
//! * Flip-flops with enable and reset are represented directly, not as primary inputs and outputs.
//!
//! # Features
//!
//! Quaigh features bounded equivalence checking, AIG simplification and basic ATPG.
//! At the moment, these are far from state of the art: for production designs, please use ABC for logic simplification
//! and Atalanta for test pattern generation.
//!
//! # Usage
//!
//! ```bash
//! # Show available commands
//! quaigh help
//! # At the moment, only .bench files are supported
//! quaigh optimize mydesign.bench -o optimized.bench
//! ```
//!
//! Quaigh is not published on crates.io yet, but you can install it from the git repository using Cargo:
//! ```bash
//! cargo install --git https://github.com/Coloquinte/quaigh
//! ```

#![warn(missing_docs)]

mod network;

pub mod equiv;
pub mod io;
pub mod sim;

pub use network::{area, generators, stats, Aig, Gate, NaryType, Signal};
