/// Representation of a fault, with its type and location
pub enum Fault {
    /// Output stuck-at fault: the output of the given gate is stuck at a fixed value
    OutputStuckAtFault {
        /// Gate where the fault is located
        gate: usize,
        /// Fault value
        value: bool,
    },
}
