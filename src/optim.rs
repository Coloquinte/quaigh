//! Optimization of logic networks

use crate::Network;

/// Completely flatten And and Xor gates in a network
///
/// Gates will be completely merged. This can result in very large And and Xor gates which share many inputs.
/// To avoid quadratic blowup, a maximum size can be specified. Gates that do not share inputs will be
/// flattened regardless of their size
pub fn flatten_nary(aig: &Network, max_size: usize) -> Network {
    let ret = Network::new();
    ret
}

/// Factor And or Xor gates with common inputs
///
/// Transform large gates into binary
pub fn factor_nary(aig: &mut Network) -> Network {
    let ret = Network::new();
    ret
}
