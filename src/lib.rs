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
//! Quaigh is written in Rust. It is now published on crates.io, and can be installed using
//! [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html), Rust's package manager:
//! ```bash
//! cargo install quaigh
//! ```
//!
//! # Development
//!
//! ## Philosophy
//!
//! In most logic optimization libraries, there are many different datastructures depending on the kind of logic
//! representation that is optimized: AIG, MIG, LUT, ...
//! Depending on the circuit, one view or the other might be preferable.
//! Different but similar datastructures exist for multiple tasks.
//! Taking advantage of them all may require splitting the circuit, making most operations much more complex.
//! On the other hand, more generic netlists will allow all kind of logic gates in a single datastructure.
//! Since they do not restrict the functions represented, they are too difficult to work with for logic optimization.
//!
//! In Quaigh, all algorithms operate on a single datastructure, `Network`.
//! This makes it possible to compare representations using completely different gates.
//! An algorithm targeting And gates (for example) can ignore everything else.
//!
//! ## Datastructures
//!
//! `Network` is a typical Gate-Inverter-Graph representation of a logic circuit.
//! Inverters are implicit, occupying just one bit in `Signal`.
//! It supports many kinds of logic, and all can coexist in the same circuit:
//! * Complex gates such as Xor, Mux and Maj3 are all first class citizens;
//! * Flip-flops with enable and reset are represented directly.
//!
//! Since the structure targets logic optimization, it maintains some limitations to make algorithms simpler.
//! All gates have a single output, representing a single binary value.
//! The network is kept in topological order, so that a given gate has an index higher than its inputs.
//!
//! For example, here is a full adder circuit:
//! ```
//! # use quaigh::{Gate, Network};
//! let mut net = Network::new();
//! let i0 = net.add_input();
//! let i1 = net.add_input();
//! let i2 = net.add_input();
//! let carry = net.add(Gate::Maj(i0, i1, i2));
//! let out = net.add(Gate::Xor3(i0, i1, i2));
//! net.add_output(carry);
//! net.add_output(out);
//! ```
//!
//! ## Library and documentation
//!
//! Quaigh is distributed as a library as well as a binary program.
//! The library is not stable, and the datastructures may change between versions.
//! Its documentation is available on [docs.rs](https://docs.rs/crate/quaigh/latest).

#![warn(missing_docs)]

pub mod atpg;
pub mod equiv;
pub mod io;
pub mod network;
pub mod optim;
pub mod sim;

pub use network::{area, stats, Gate, NaryType, Network, Signal};
