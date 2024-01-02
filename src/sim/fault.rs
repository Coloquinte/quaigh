use crate::Network;

/// Representation of a fault, with its type and location
#[derive(Clone, Copy, Debug)]
pub enum Fault {
    /// Output stuck-at fault: the output of the given gate is stuck at a fixed value
    OutputStuckAtFault {
        /// Gate where the fault is located
        gate: usize,
        /// Fault value
        value: bool,
    },
}

impl Fault {
    /// Get all possible faults in a network
    pub fn all(aig: &Network) -> Vec<Fault> {
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
        }
        ret
    }
}
