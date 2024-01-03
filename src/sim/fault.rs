use crate::{network::stats, Gate, Network};

/// Representation of a fault, with its type and location
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
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
        let usage = stats::count_gate_usage(aig);
        let mut ret = Vec::new();
        for i in 0..aig.nb_nodes() {
            ret.push(Fault::OutputStuckAtFault {
                gate: i,
                value: false,
            });
            ret.push(Fault::OutputStuckAtFault {
                gate: i,
                value: true,
            });
            if let Gate::Buf(_) = aig.gate(i) {
                // Input stuck for a buffer is already covered by the output fault
                continue;
            }
            for (j, s) in aig.gate(i).dependencies().iter().enumerate() {
                if s.is_var() && usage[s.var() as usize] == 1 {
                    // No need to handle input stuck fault if this gate is the only user
                    continue;
                }
                ret.push(Fault::InputStuckAtFault {
                    gate: i,
                    input: j,
                    value: false,
                });
                ret.push(Fault::InputStuckAtFault {
                    gate: i,
                    input: j,
                    value: true,
                });
            }
        }
        // TODO: mark redundant faults above, and remove them here
        // TODO: handle Buf-like gates, where the previous output fault is redundant if used once
        // TODO: handle And-like gates, where one of the stuck directions is redundant with the output fault
        // TODO: handle Xor-like gates, where the errors are covered by the output faults
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
