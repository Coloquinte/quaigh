//! Representation and handling of logic networks

pub mod area;
mod gates;
pub mod generators;
mod network;
mod signal;
pub mod stats;

pub use gates::{BinaryType, Gate, NaryType, TernaryType};
pub use network::Network;
pub use signal::Signal;
