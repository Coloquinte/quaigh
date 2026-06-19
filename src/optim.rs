//! Optimization of logic networks

mod aig;
mod balance;
pub mod cuts;
mod infer_gates;
mod mig;
mod share_logic;

pub use aig::to_aig;
pub use balance::{balance, balance_with};
pub use cuts::{enumerate_cuts, enumerate_cuts_with, is_valid_cut, Cut};
pub use infer_gates::{infer_dffe, infer_xor_mux};
pub use mig::to_mig;
pub use share_logic::share_logic;
