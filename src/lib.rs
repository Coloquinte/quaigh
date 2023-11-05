//! Implementation of And-Inverter-Graph for logic simplification (AIG).
//!
//! This crate provides a flexible representation for logic graphs based on And, Mux and Majority gates with implicit inverters.
//! It provides utilities to manipulate and simplify logic functions, check for equivalence, ...
//!
//! It is inspired by the logic synthesis tools [ABC](https://people.eecs.berkeley.edu/~alanmi/abc/) and [Mockturtle](https://mockturtle.readthedocs.io/en/latest/).
//! Our goal is to provide an easy-to-use library for logical synthesis and technology mapping, and improve its quality over time.
//!
//! # Design
//!
//! Quaigh supports more complex logic gates than the traditional AIG representation.
//! And2, Xor2, And3, Xor3, Mux (multiplexer) and Maj3 (majority) gates are all first class citizens.
//! Contrary to many similar representation (MIG/XAG/MuxIG), all these gates can coexist in the same logic circuit.
//! Circuits using complex gates will usually be much more compact as a result.
//!
//! # Features
//!
//! Quaigh features bounded equivalence checking and AIG simplification.
//! At the moment, these are far from state of the art: for production designs, please use ABC.

#![warn(missing_docs)]

mod aig;
mod gates;
mod signal;

pub mod equiv;

pub use aig::Aig;
pub use gates::Gate;
pub use signal::Signal;
