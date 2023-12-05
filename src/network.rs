//! Representation and handling of logic networks

mod aig;
pub mod area;
mod gates;
pub mod generators;
mod signal;
pub mod stats;

pub use aig::Aig;
pub use gates::{Gate, NaryType};
pub use signal::Signal;
