use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::network::stats;
use crate::Network;

use super::simple_sim::SimpleSimulator;
use super::Fault;

/// Structure for simulation that only touches the values that were modified
pub struct IncrementalSimulator<'a> {
    /// Whether a gate is an output
    is_output: Vec<bool>,
    /// Gates that use each gate
    gate_users: Vec<Vec<usize>>,
    /// Simple simulator for the initial simulation
    sim: SimpleSimulator<'a>,
    /// Simulator that will be updated incrementally
    incr_sim: SimpleSimulator<'a>,
    /// Queue of nodes to update, lowest index first
    update_queue: BinaryHeap<Reverse<usize>>,
    /// List of modified value
    modified_values: Vec<usize>,
    /// Whether each value is modified
    is_modified: Vec<bool>,
}

impl<'a> IncrementalSimulator<'a> {
    /// Build a simulator by capturing a network
    pub fn from_aig(aig: &'a Network) -> IncrementalSimulator<'a> {
        assert!(aig.is_topo_sorted());
        let sim = SimpleSimulator::from_aig(aig);
        let incr_sim = sim.clone();
        IncrementalSimulator {
            is_output: stats::gate_is_output(aig),
            gate_users: stats::gate_users(aig),
            sim,
            incr_sim,
            update_queue: BinaryHeap::new(),
            modified_values: Vec::new(),
            is_modified: vec![false; aig.nb_nodes()],
        }
    }

    /// Reset the state of the simulator
    fn reset(&mut self) {
        for v in &self.modified_values {
            self.incr_sim.node_values[*v] = self.sim.node_values[*v];
            self.is_modified[*v] = false;
        }
        self.update_queue.clear();
        self.modified_values.clear();
    }

    /// Run the simulation from a fault
    pub fn run_initial(&mut self, input_values: &Vec<u64>) {
        self.sim.reset();
        self.sim.copy_inputs(input_values);
        self.sim.run_comb();
        self.incr_sim = self.sim.clone();
    }

    /// Update a single gate
    fn update_gate(&mut self, i: usize, value: u64) {
        let old_val = self.incr_sim.node_values[i];
        if old_val == value {
            return;
        }
        self.incr_sim.node_values[i] = value;
        self.modified_values.push(i);
        self.is_modified[i] = true;
        for &j in &self.gate_users[i] {
            self.update_queue.push(Reverse(j));
        }
    }

    /// Run the simulation from a fault
    fn run_incremental(&mut self, fault: Fault) {
        match fault {
            Fault::OutputStuckAtFault { gate, value } => {
                self.update_gate(gate, if value { !0 } else { 0 });
            }
            Fault::InputStuckAtFault { gate, input, value } => {
                let value = self.incr_sim.run_gate_with_input_stuck(gate, input, value);
                self.update_gate(gate, value)
            }
        }
        while let Some(Reverse(i)) = self.update_queue.pop() {
            let v = self.incr_sim.run_gate(i);
            self.update_gate(i, v);
        }
    }

    /// Whether an output has been modified by the incremental run
    fn output_modified(&self) -> u64 {
        let mut ret = 0;
        for i in &self.modified_values {
            if self.is_output[*i] {
                ret |= self.incr_sim.node_values[*i] ^ self.sim.node_values[*i];
            }
        }
        ret
    }

    /// Whether the given fault is detected by the pattern
    pub fn detects_fault(&mut self, fault: Fault) -> u64 {
        self.run_incremental(fault);
        let ret = self.output_modified();
        self.reset();
        ret
    }
}
