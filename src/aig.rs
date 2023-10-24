use crate::gates::Gate;
use crate::gates::Normalization;
use crate::signal::Signal;

#[derive(Debug, Clone, Default)]
pub struct Aig {
    nb_inputs: usize,
    nodes: Vec<Gate>,
    outputs: Vec<Signal>,
}

impl Aig {
    /**
     * Create a new Aig
     */
    pub fn new() -> Self {
        Self::default()
    }

    /**
     * Return the number of primary inputs of the AIG
     */
    pub fn nb_inputs(&self) -> usize {
        self.nb_inputs
    }

    /**
     * Return the number of primary outputs of the AIG
     */
    pub fn nb_outputs(&self) -> usize {
        self.outputs.len()
    }

    /**
     * Return the number of nodes in the AIG
     */
    pub fn nb_nodes(&self) -> usize {
        self.nodes.len()
    }

    /**
     * Get the input at index i
     */
    pub fn input(&self, i: usize) -> Signal {
        assert!(i < self.nb_inputs());
        Signal::from_input(i as u32)
    }

    /**
     * Get the output at index i
     */
    pub fn output(&self, i: usize) -> Signal {
        assert!(i < self.nb_outputs());
        self.outputs[i]
    }

    /**
     * Get the gate at index i
     */
    pub fn node(&self, i: usize) -> Gate {
        self.nodes[i]
    }

    /**
     * Number of And2 gates
     */
    pub fn nb_and(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::And(_, _)))
            .count()
    }

    /**
     * Number of Xor2 gates
     */
    pub fn nb_xor(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::Xor(_, _)))
            .count()
    }

    /**
     * Number of And3 gates
     */
    pub fn nb_and3(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::And3(_, _, _)))
            .count()
    }

    /**
     * Number of Xor3 gates
     */
    pub fn nb_xor3(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::Xor3(_, _, _)))
            .count()
    }

    /**
     * Number of Mux gates
     */
    pub fn nb_mux(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::Mux(_, _, _)))
            .count()
    }

    /**
     * Number of Maj gates
     */
    pub fn nb_maj(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::Maj(_, _, _)))
            .count()
    }

    /**
     * Number of Dff gates
     */
    pub fn nb_dff(&self) -> usize {
        self.nodes
            .iter()
            .filter(|g| matches!(g, Gate::Dff(_, _, _)))
            .count()
    }

    /**
     * Add a new primary input
     */
    pub fn add_input(&mut self) -> Signal {
        self.nb_inputs += 1;
        self.input(self.nb_inputs() - 1)
    }

    /**
     * Add a new primary output based on an existing literal
     */
    pub fn add_output(&mut self, l: Signal) {
        self.outputs.push(l)
    }

    /**
     * Create an And2 gate
     */
    pub fn and(&mut self, a: Signal, b: Signal) -> Signal {
        self.add_gate(Gate::And(a, b))
    }

    /**
     * Create an Or2 gate
     */
    pub fn or(&mut self, a: Signal, b: Signal) -> Signal {
        !self.and(!a, !b)
    }

    /**
     * Create a Xor2 gate
     */
    pub fn xor(&mut self, a: Signal, b: Signal) -> Signal {
        self.add_gate(Gate::Xor(a, b))
    }

    /**
     * Create an And3 gate
     */
    pub fn and3(&mut self, a: Signal, b: Signal, c: Signal) -> Signal {
        self.add_gate(Gate::And3(a, b, c))
    }

    /**
     * Create an Or3 gate
     */
    pub fn or3(&mut self, a: Signal, b: Signal, c: Signal) -> Signal {
        !self.and3(!a, !b, !c)
    }

    /**
     * Create a Xor3 gate
     */
    pub fn xor3(&mut self, a: Signal, b: Signal, c: Signal) -> Signal {
        self.add_gate(Gate::Xor3(a, b, c))
    }

    /**
     * Create a Mux gate
     */
    pub fn mux(&mut self, s: Signal, a: Signal, b: Signal) -> Signal {
        self.add_gate(Gate::Mux(s, a, b))
    }

    /**
     * Create a Maj gate
     */
    pub fn maj(&mut self, a: Signal, b: Signal, c: Signal) -> Signal {
        self.add_gate(Gate::Maj(a, b, c))
    }

    /**
     * Create a Dff gate (flip flop)
     */
    pub fn dff(&mut self, d: Signal, en: Signal, res: Signal) -> Signal {
        self.add_gate(Gate::Dff(d, en, res))
    }

    /**
     * Add a new gate, normalized
     */
    fn add_gate(&mut self, gate: Gate) -> Signal {
        use Normalization::*;
        let g = gate.make_canonical();
        match g {
            Buf(l) => l,
            Node(g, inv) => self.add_raw_gate(g) ^ inv,
        }
    }

    /**
     * Add a new gate, without normalization
     */
    fn add_raw_gate(&mut self, gate: Gate) -> Signal {
        let l = Signal::from_var(self.nodes.len() as u32);
        self.nodes.push(gate);
        l
    }

    /**
     * Return whether the AIG is purely combinatorial
     */
    pub fn is_comb(&self) -> bool {
        self.nb_dff() == 0
    }

    /**
     * Return whether the AIG is already topologically sorted (except for flip-flops)
     */
    fn is_topo_sorted(&self) -> bool {
        use Gate::*;
        for (i, g) in self.nodes.iter().enumerate() {
            let ind = i as u32;
            match g {
                And(a, b) | Xor(a, b) => {
                    if a.is_var() && a.ind() > ind {
                        return false;
                    }
                    if b.is_var() && b.ind() > ind {
                        return false;
                    }
                }
                And3(a, b, c) | Xor3(a, b, c) | Mux(a, b, c) | Maj(a, b, c) => {
                    if a.is_var() && a.ind() > ind {
                        return false;
                    }
                    if b.is_var() && b.ind() > ind {
                        return false;
                    }
                    if c.is_var() && c.ind() > ind {
                        return false;
                    }
                }
                Dff(_, _, _) => (),
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{Aig, Signal};

    #[test]
    fn test_basic() {
        let mut aig = Aig::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let x = aig.xor(i0, i1);
        aig.add_output(x);

        // Basic properties
        assert_eq!(aig.nb_inputs(), 2);
        assert_eq!(aig.nb_outputs(), 1);
        assert_eq!(aig.nb_nodes(), 1);
        assert!(aig.is_comb());
        assert!(aig.is_topo_sorted());

        // Access
        assert_eq!(aig.input(0), i0);
        assert_eq!(aig.input(1), i1);
        assert_eq!(aig.output(1), x);
    }

    #[test]
    fn test_dff() {
        let mut aig = Aig::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let c0 = Signal::zero();
        let c1 = Signal::one();
        // Useful Dff
        assert_eq!(aig.dff(i0, i1, i2), Signal::from_var(0));
        assert_eq!(aig.dff(i0, i1, c0), Signal::from_var(1));
        // Dff that reduces to 0
        assert_eq!(aig.dff(c0, i1, i2), c0);
        assert_eq!(aig.dff(i0, c0, i2), c0);
        assert_eq!(aig.dff(i0, i1, c1), c0);
        assert!(!aig.is_comb());
        assert!(aig.is_topo_sorted());
    }
}
