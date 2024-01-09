//! Logic simplification and analysis tools
//!
//! This crate provides tools for logic optimization, synthesis, technology mapping and analysis.
//! Our goal is to provide an easy-to-use library, and improve its quality over time to match industrial tools.
//!
//! # Usage and features
//!
//! Quaigh provides a command line tool, that can be installed using
//! [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):
//! `cargo install quaigh`.
//!
//! To show available commands:
//! ```bash
//! quaigh help
//! ```
//!
//! The `atpg` command performs [automatic test pattern generation](https://en.wikipedia.org/wiki/Automatic_test_pattern_generation),
//! to create test vectors for a design.
//! ```bash
//! quaigh atpg mydesign.bench -o atpg.test
//! ```
//!
//! The `check-equivalence` command performs bounded [equivalence checking](https://en.wikipedia.org/wiki/Formal_equivalence_checking)
//! to confirm that a design's functionality is preserved after transformations.
//! ```bash
//! quaigh equiv mydesign.bench optimized.bench
//! ```
//!
//! The `optimize` command performs [logic optimization](https://en.wikipedia.org/wiki/Logic_optimization).
//! At the moment, logic optimization is far from state of the art: for production designs, you should
//! generally stick to the tools included in [Yosys](https://github.com/YosysHQ/yosys).
//! ```bash
//! quaigh opt mydesign.bench -o optimized.bench
//! ```
//!
//! At the moment, the only supported input format is `.bench`. Benchmarks in .bench and .blif format can be downloaded
//! [here](https://github.com/Coloquinte/moosic-yosys-plugin/releases/download/iscas_benchmarks/benchmarks.tar.xz).
//! More features will be added over time, such as technology mapping, operator optimization, ...
//! The complete documentation is available on [docs.rs](https://docs.rs/crate/quaigh/latest).
//!
//! # Development
//!
//! ## Philosophy
//!
//! In most logic optimization libraries ([ABC](https://github.com/berkeley-abc/abc), [Mockturtle](https://github.com/lsils/mockturtle), ...),
//! there are many different datastructures depending on the kind of logic representation that is optimized:
//! AIG, MIG, LUT, ...
//! Depending on the circuit, one view or the other might be preferable.
//! Taking advantage of them all may require [splitting the circuit](https://github.com/lnis-uofu/LSOracle), making most operations much more complex.
//! More generic netlists, like [Yosys RTLIL](https://yosyshq.readthedocs.io/projects/yosys/en/latest/CHAPTER_Overview.html#the-rtl-intermediate-language-rtlil),
//! will allow all kind of logic gates in a single datastructure.
//! Since they do not restrict the functions represented, they are difficult to work directly for logic optimization.
//!
//! Quaigh aims in-between. All algorithms operate on a single datastructure, `Network`.
//! This makes it possible to compare representations using completely different gates.
//! An algorithm targeting And gates (for example) can ignore everything else.
//! Compared to a netlist datastructure, it is flat and focuses completely on logic optimization.
//!
//! ## Datastructures
//!
//! `Network` is a typical Gate-Inverter-Graph representation of a logic circuit.
//! Inverters are implicit, occupying just one bit in `Signal`.
//! It supports many kinds of logic, and all can coexist in the same circuit:
//! *   Complex gates such as Xor, Mux and Maj3 are all first class citizens;
//! *   Flip-flops with enable and reset are represented directly.
//!
//! Since the structure targets logic optimization, it maintains some limitations to make algorithms simpler.
//! All gates have a single output, representing a single binary value.
//! The network is kept in topological order, so that a given gate has an index higher than its inputs.
//! Finally, it does not attempt to handle names or design hierarchy.
//!
//! For example, here is a full adder circuit:
//! ```
//! # use quaigh::{Gate, Network};
//! let mut net = Network::new();
//! let i0 = net.add_input();
//! let i1 = net.add_input();
//! let i2 = net.add_input();
//! let carry = net.add(Gate::maj(i0, i1, i2));
//! let out = net.add(Gate::xor3(i0, i1, i2));
//! net.add_output(carry);
//! net.add_output(out);
//! ```

#![warn(missing_docs)]

pub mod atpg;
pub mod equiv;
pub mod io;
pub mod network;
pub mod optim;
pub mod sim;

pub use network::{Gate, NaryType, Network, Signal};
