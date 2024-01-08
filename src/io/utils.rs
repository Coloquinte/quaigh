use std::collections::HashSet;

use crate::{Gate, Network, Signal};

/// Ad-hoc to_string function to represent signals in bench files
pub fn sig_to_string(s: &Signal) -> String {
    if *s == Signal::one() {
        return "vdd".to_string();
    }
    if *s == Signal::zero() {
        return "gnd".to_string();
    }
    s.without_inversion().to_string() + (if s.is_inverted() { "_n" } else { "" })
}

/// Find the set of signals that are used inverted
pub fn get_inverted_signals(aig: &Network) -> Vec<Signal> {
    // Generate signals where the inversion is required
    let mut signals_with_inv = HashSet::new();
    for o in 0..aig.nb_outputs() {
        let s = aig.output(o);
        if s.is_inverted() && !s.is_constant() {
            signals_with_inv.insert(!s);
        }
    }
    for i in 0..aig.nb_nodes() {
        if matches!(aig.gate(i), Gate::Buf(_)) {
            // Buf(!x) is exported directly as a Not
            continue;
        }
        for s in aig.gate(i).dependencies() {
            if s.is_inverted() && !s.is_constant() {
                signals_with_inv.insert(!s);
            }
        }
    }
    let mut signals_with_inv = signals_with_inv.into_iter().collect::<Vec<_>>();
    signals_with_inv.sort();
    signals_with_inv
}
