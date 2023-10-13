use crate::aig_node::AigNode;
use crate::literal::Lit;

#[derive(Debug, Clone)]
pub struct Aig {
    nb_inputs: usize,
    nodes: Vec<AigNode>,
    outputs: Vec<Lit>,
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
    pub fn input(&self, i: usize) -> Lit {
        Lit::zero()
    }

    /**
     * Get the output at index i
     */
    pub fn output(&self, i: usize) -> Lit {
        self.outputs[i]
    }

    /**
     * Add a new primary input
     */
    pub fn add_input(&mut self) -> Lit {
        Lit::zero()
    }

    /**
     * Add a new primary output based on an existing literal
     */
    pub fn add_output(&mut self, l: Lit) {
        self.outputs.push(l)
    }

    pub fn and(&mut self, a: Lit, b: Lit) -> Lit {
        self.maj(a, b, Lit::zero())
    }

    pub fn or(&mut self, a: Lit, b: Lit) -> Lit {
        !self.and(!a, !b)
    }

    pub fn xor(&mut self, a: Lit, b: Lit) -> Lit {
        self.mux(a, !b, b)
    }

    pub fn mux(&mut self, s: Lit, a: Lit, b: Lit) -> Lit {
        Lit::zero()
        // TODO
    }

    pub fn maj(&mut self, a: Lit, b: Lit, c: Lit) -> Lit {
        Lit::zero()
        // TODO
    }

    /**
     * Return whether the AIG is purely combinatorial
     */
    pub fn is_comb(&self) -> bool {
        true
    }

    /**
     * Cleanup the AIG with simple canonization/sorting/duplicate removal. Note that all literals will be invalidated
     */
    pub fn cleanup(&self) -> Self {
        self.clone()
        // TODO
    }
}
