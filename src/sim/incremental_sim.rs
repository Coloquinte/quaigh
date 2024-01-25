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
    touched_gates: Vec<usize>,
    /// Whether each value is on the queue
    is_touched: Vec<bool>,
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
            touched_gates: Vec::new(),
            is_touched: vec![false; aig.nb_nodes()],
        }
    }

    /// Reset the state of the simulator
    fn reset(&mut self) {
        for v in &self.touched_gates {
            self.incr_sim.node_values[*v] = self.sim.node_values[*v];
            self.is_touched[*v] = false;
        }
        assert!(self.update_queue.is_empty());
        self.touched_gates.clear();
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
        if !self.is_touched[i] {
            // Check it explicitly for the first gate
            self.is_touched[i] = true;
            self.touched_gates.push(i);
        }
        self.incr_sim.node_values[i] = value;
        for &j in &self.gate_users[i] {
            if !self.is_touched[j] {
                self.is_touched[j] = true;
                self.update_queue.push(Reverse(j));
                self.touched_gates.push(j);
            }
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
                self.update_gate(gate, value);
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
        for i in &self.touched_gates {
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
