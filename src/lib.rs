//! Implementation of And-Inverter-Graph for logic simplification (AIG).
//!
//! This crate provides a flexible representation for logic graphs based on And, Mux and Majority gates with implicit inverters.
//! It provides utilities to manipulate and simplify logic functions, check for equivalence, ...
//!
//! It is inspired by the logic synthesis tools ABC and Mockturtle.
//! Our goal is to become competitive with ABC for industrial applications, but with ease-of-use as a primary goal.

mod aig_node;
mod literal;
mod aig;


pub use literal::Lit;
pub use aig_node::AigNode;
pub use aig::Aig;