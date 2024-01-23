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
//! Quaigh supports a subset of the [Blif](https://course.ece.cmu.edu/~ee760/760docs/blif.pdf) file format, as well
//! as the simple Bench file format used by ISCAS benchmarks. Benchmarks can be downloaded
//! [here](https://github.com/Coloquinte/moosic-yosys-plugin/releases/download/iscas_benchmarks/benchmarks.tar.xz).
//! More features will be added over time, such as technology mapping, operator optimization, ...
//! The complete documentation is available on [docs.rs](https://docs.rs/crate/quaigh/latest).
//!
//! # Development
//!
//! The main datastructure, [`Network`](https://docs.rs/quaigh/latest/quaigh/network/struct.Network.html), is a typical Gate-Inverter-Graph representation of a logic circuit.
//! Inverters are implicit, occupying just one bit in [`Signal`](https://docs.rs/quaigh/latest/quaigh/network/struct.Signal.html).
//! It supports many kinds of logic, and all can coexist in the same circuit:
//! *   Complex gates such as Xor, Mux and Maj3 are all first class citizens;
//! *   Flip-flops with enable and reset are represented directly.
//!
//! In most logic optimization libraries ([ABC](https://github.com/berkeley-abc/abc), [Mockturtle](https://github.com/lsils/mockturtle), ...),
//! there are many different ways to represent logic, with separate datastructures: AIG, MIG, LUT, ...
//! Depending on the circuit, one view or the other might be preferable.
//! Taking advantage of them all may require [splitting the circuit](https://github.com/lnis-uofu/LSOracle), making most operations much more complex.
//! More generic netlists, like [Yosys RTLIL](https://yosyshq.readthedocs.io/projects/yosys/en/latest/CHAPTER_Overview.html#the-rtl-intermediate-language-rtlil),
//! will allow all kind of logic gates in a single datastructure.
//! Since they do not restrict the functions represented, they are difficult to work with directly for logic optimization.
//!
//! Quaigh aims in-between. All algorithms share the same netlist representation, [`Network`](https://docs.rs/quaigh/latest/quaigh/network/struct.Network.html),
//! but there are some limitations to make it easy to optimize:
//! *   all gates have a single output, representing a single binary value,
//! *   the gates are kept in topological order (a gate has an index higher than its inputs),
//! *   names and design hierarchy are not represented.
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
//!
//! Apart from the core datastructure, Quaigh has algorithms for [logic optimization](https://docs.rs/quaigh/latest/quaigh/optim/index.html),
//! [simulation](https://docs.rs/quaigh/latest/quaigh/sim/index.html) (including fault simulation) and
//! [test pattern generation](https://docs.rs/quaigh/latest/quaigh/atpg/index.html).
//! For optimization and equivalence checking, Quaigh relies on other packages as much as possible:
//! *   [Kissat](https://github.com/arminbiere/kissat) (using [rustsat](https://docs.rs/rustsat/)) as a Sat solver,
//! *   [Highs](https://github.com/ERGO-Code/HiGHS) (using [good_lp](https://docs.rs/good_lp/)) as an optimization solver.

#![warn(missing_docs)]

pub mod atpg;
pub mod equiv;
pub mod io;
pub mod network;
pub mod optim;
pub mod sim;

pub use network::{Gate, Network, Signal};
