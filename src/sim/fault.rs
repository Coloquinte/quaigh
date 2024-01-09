use std::fmt;

use crate::network::{stats, NaryType};
use crate::{Gate, Network, Signal};

/// Representation of a fault, with its type and location
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fault {
    /// Output stuck-at fault: the output of the given gate is stuck at a fixed value
    OutputStuckAtFault {
        /// Gate where the fault is located
        gate: usize,
        /// Fault value
        value: bool,
    },
    /// Input stuck-at fault: the input of the given gate is stuck at a fixed value
    InputStuckAtFault {
        /// Gate where the fault is located
        gate: usize,
        /// Input where the fault is located
        input: usize,
        /// Fault value
        value: bool,
    },
}

impl Fault {
    /// Get all possible faults in a network
    pub fn all(aig: &Network) -> Vec<Fault> {
        let mut ret = Vec::new();
        for gate in 0..aig.nb_nodes() {
            for value in [false, true] {
                ret.push(Fault::OutputStuckAtFault { gate, value });
            }
            for input in 0..aig.gate(gate).dependencies().len() {
                for value in [false, true] {
                    ret.push(Fault::InputStuckAtFault { gate, input, value });
                }
            }
        }
        ret
    }

    /// Get all possible non-redundant faults in a network
    pub fn all_unique(aig: &Network) -> Vec<Fault> {
        let mut ret = Fault::all(aig);
        let redundant = Fault::redundant_faults(aig);
        ret.retain(|f| !redundant.binary_search(f).is_ok());
        ret
    }

    /// List the redundant faults in a network
    ///
    /// A fault is redundant if it is covered by other faults.
    /// The redundancy found here must be acyclic, so that we do not discard a group of equivalent faults.
    /// When determining redundancy, we always keep the output stuck-at fault, and if equivalent
    /// faults are the same type we keep the later one.
    pub fn redundant_faults(aig: &Network) -> Vec<Fault> {
        let usage = stats::count_gate_usage(aig);
        // Returns whether the signal is a variable that is used once, so that its input stuck-at fault and output stuck-at fault are equivalent
        let is_single_use = |s: &Signal| -> bool { s.is_var() && usage[s.var() as usize] <= 1 };
        let mut ret = Vec::new();
        for gate in 0..aig.nb_nodes() {
            let g = aig.gate(gate);
            for (input, s) in g.dependencies().iter().enumerate() {
                for value in [false, true] {
                    if is_single_use(s) {
                        // Fault covered by a previous output stuck-at fault, because the output is used only once
                        ret.push(Fault::InputStuckAtFault { gate, input, value });
                    }
                    if g.is_xor_like() || g.is_buf_like() {
                        // Fault redundant because this is a Xor-like gate: it is equivalent to faults on the output
                        ret.push(Fault::InputStuckAtFault { gate, input, value });
                        if is_single_use(s) {
                            ret.push(Fault::OutputStuckAtFault {
                                gate: s.var() as usize,
                                value,
                            });
                        }
                    }
                    if g.is_and_like() {
                        // Some faults are redundant because this is an And-like gate: one of the values forces the gate
                        let input_inv = matches!(
                            g,
                            Gate::Nary(_, NaryType::Or) | Gate::Nary(_, NaryType::Nor)
                        );
                        if value == input_inv {
                            ret.push(Fault::InputStuckAtFault { gate, input, value });
                            if is_single_use(s) {
                                ret.push(Fault::OutputStuckAtFault {
                                    gate: s.var() as usize,
                                    value,
                                });
                            }
                        }
                    }
                }
            }
        }
        ret.sort();
        ret.dedup();
        ret
    }

    /// Return true if there are two faults with the same gate in the vector
    pub fn has_duplicate_gate(faults: &Vec<Fault>) -> bool {
        let mut gates = Vec::new();
        for f in faults {
            match f {
                Fault::OutputStuckAtFault { gate, .. } => gates.push(*gate),
                Fault::InputStuckAtFault { gate, .. } => gates.push(*gate),
            }
        }
        gates.sort();
        for i in 1..gates.len() {
            if gates[i - 1] == gates[i] {
                return true;
            }
        }
        false
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::OutputStuckAtFault { gate, value } => {
                write!(f, "Gate {} output stuck at {}", gate, i32::from(*value))
            }
            Fault::InputStuckAtFault { gate, input, value } => {
                write!(
                    f,
                    "Gate {} input {} stuck at {}",
                    gate,
                    input,
                    i32::from(*value)
                )
            }
        }
    }
}
