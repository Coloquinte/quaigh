use crate::gates::Gate;
use crate::gates::Normalization;
use crate::signal::Signal;

#[derive(Debug, Clone)]
pub struct Aig {
    nb_inputs: usize,
    nodes: Vec<Gate>,
    outputs: Vec<Signal>,
}

impl Aig {
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
        Signal::from_ind(!(i as u32 + 1))
    }

    /**
     * Get the output at index i
     */
    pub fn output(&self, i: usize) -> Signal {
        self.outputs[i]
    }

    /**
     * Get the gate at index i
     */
    pub fn node(&self, i: usize) -> Gate {
        self.nodes[i].into()
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
     * Create an and gate
     */
    pub fn and(&mut self, a: Signal, b: Signal) -> Signal {
        self.add_gate(Normalization::Node(Gate::And(a, b), false))
    }

    /**
     * Create an or gate
     */
    pub fn or(&mut self, a: Signal, b: Signal) -> Signal {
        !self.and(!a, !b)
    }

    /**
     * Create an xor gate
     */
    pub fn xor(&mut self, a: Signal, b: Signal) -> Signal {
        self.add_gate(Normalization::Node(Gate::Xor(a, b), false))
    }

    /**
     * Create a mux gate
     */
    pub fn mux(&mut self, s: Signal, a: Signal, b: Signal) -> Signal {
        self.add_gate(Normalization::Node(Gate::Mux(s, a, b), false))
    }

    /**
     * Create an maj gate
     */
    pub fn maj(&mut self, a: Signal, b: Signal, c: Signal) -> Signal {
        self.add_gate(Normalization::Node(Gate::Maj(a, b, c), false))
    }

    /**
     * Create a dff gate
     */
    pub fn dff(&mut self, d: Signal, en: Signal, res: Signal) -> Signal {
        self.add_gate(Normalization::Node(Gate::Dff(d, en, res), false))
    }

    fn add_gate(&mut self, gate: Normalization) -> Signal {
        use Normalization::*;
        let g = gate.make_canonical();
        match g {
            Buf(l) => l,
            Node(g, inv) => {
                let l = Signal::from_var(self.nodes.len() as u32);
                self.nodes.push(g);
                l ^ inv
            }
        }
    }

    /**
     * Return whether the AIG is purely combinatorial
     */
    pub fn is_comb(&self) -> bool {
        true
    }

    /**
     * Cleanup the AIG with simple canonization/sorting/duplicate removal
     *
     * Note that all literals will be invalidated, and only the number of inputs/outputs stays the same
     */
    pub fn cleanup(&self) -> Self {
        self.clone()
        // TODO
    }

    /**
     * Convert the AIG to a restricted representation, replacing complex gates by And gates
     */
    pub fn restrict_gates(&self, allow_xor: bool, allow_mux: bool, allow_maj: bool) -> Self {
        self.clone()
        // TODO
    }
}
