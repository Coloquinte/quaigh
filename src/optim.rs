//! Optimization of logic networks

mod infer_gates;
mod share_logic;

pub use infer_gates::infer_xor_mux;
pub use share_logic::share_logic;
