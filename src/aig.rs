use crate::aig_node::AigNode;
use crate::literal::Lit;

pub struct Aig {
    nb_inputs: usize,
    nodes: Vec<AigNode>,
    outputs: Vec<Lit>,
}

impl Aig {
    /**
     * Return the number of primary inputs of the AIG
     */
    pub fn nb_inputs(&self) -> usize { self.nb_inputs }

    /**
     * Return the number of primary outputs of the AIG
     */
    pub fn nb_outputs(&self) -> usize { self.outputs.len() }

    /**
     * Return the number of nodes in the AIG
     */
    pub fn nb_nodes(&self) -> usize { self.nodes.len() }

    /**
     * Return whether the AIG is purely combinatorial
     */
    pub fn is_comb(&self) -> bool { true }

    /**
     * Cleanup the AIG with simple canonization/sorting/duplicate removal
     */
    pub fn cleanup(&mut self) {}
}