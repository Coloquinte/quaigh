use crate::network::{BinaryType, NaryType, TernaryType};
use crate::{Network, Signal};

use super::Fault;

/// Structure for simulation based directly on the network representation
///
/// This is simple to write and relatively efficient, but could be greatly improved
/// with a regular and- or mux-based structure.
#[derive(Clone, Debug)]
pub struct SimpleSimulator<'a> {
    aig: &'a Network,
    pub input_values: Vec<u64>,
    pub node_values: Vec<u64>,
}

/// Convert the inversion to a word for bitwise operations
fn pol_to_word(s: Signal) -> u64 {
    let pol = s.raw() & 1;
    (!(pol as u64)).wrapping_add(1)
}

/// Majority function
fn maj(a: u64, b: u64, c: u64) -> u64 {
    (b & c) | (a & (b | c))
}

/// Multiplexer function
fn mux(s: u64, a: u64, b: u64) -> u64 {
    (s & a) | (!s & b)
}

impl<'a> SimpleSimulator<'a> {
    /// Build a simulator by capturing a network
    pub fn from_aig(aig: &'a Network) -> SimpleSimulator<'a> {
        assert!(aig.is_topo_sorted());
        SimpleSimulator {
            aig,
            input_values: vec![0; aig.nb_inputs()],
            node_values: vec![0; aig.nb_nodes()],
        }
    }

    /// Run the simulation
    pub fn run(&mut self, input_values: &Vec<Vec<u64>>) -> Vec<Vec<u64>> {
        self.check();
        self.reset();
        let mut ret = Vec::new();
        for (i, v) in input_values.iter().enumerate() {
            if i != 0 {
                self.run_dff();
            }
            self.copy_inputs(v.as_slice());
            self.run_comb();
            ret.push(self.get_output_values());
        }
        ret
    }

    /// Run the simulation with a list of stuck-at-fault errors
    pub fn run_with_faults(
        &mut self,
        input_values: &Vec<Vec<u64>>,
        faults: &Vec<Fault>,
    ) -> Vec<Vec<u64>> {
        self.check();
        self.reset();
        let mut ret = Vec::new();
        for (i, v) in input_values.iter().enumerate() {
            if i != 0 {
                self.run_dff();
            }
            self.copy_inputs(v.as_slice());
            self.run_comb_with_faults(faults);
            ret.push(self.get_output_values());
        }
        ret
    }

    pub fn reset(&mut self) {
        self.input_values = vec![0; self.aig.nb_inputs()];
        self.node_values = vec![0; self.aig.nb_nodes()];
    }

    fn check(&self) {
        assert!(self.aig.is_topo_sorted());
        assert_eq!(self.input_values.len(), self.aig.nb_inputs());
        assert_eq!(self.node_values.len(), self.aig.nb_nodes());
    }

    // Get the value of a signal in the current state
    fn get_value(&self, s: Signal) -> u64 {
        if s == Signal::zero() {
            0
        } else if s == Signal::one() {
            !0
        } else if s.is_input() {
            self.input_values[s.input() as usize] ^ pol_to_word(s)
        } else {
            debug_assert!(s.is_var());
            self.node_values[s.var() as usize] ^ pol_to_word(s)
        }
    }

    // Copy the values of the inputs to the internal state
    pub fn copy_inputs(&mut self, inputs: &[u64]) {
        assert_eq!(inputs.len(), self.input_values.len());
        self.input_values.copy_from_slice(inputs);
    }

    // Copy the values of the flip-flops for the next cycle
    pub fn run_dff(&mut self) {
        use crate::Gate::*;
        let mut next_values = self.node_values.clone();
        for i in 0..self.aig.nb_nodes() {
            let g = self.aig.gate(i);
            if let Dff([d, en, res]) = g {
                let dv = self.get_value(*d);
                let env = self.get_value(*en);
                let resv = self.get_value(*res);
                let prevv = self.node_values[i];
                let val = !resv & ((env & dv) | (!env & prevv));
                next_values[i] = val;
            }
        }
        self.node_values = next_values;
    }

    /// Return the result of a single gate
    pub fn run_gate(&self, i: usize) -> u64 {
        use crate::Gate::*;
        let g = self.aig.gate(i);
        match g {
            Binary([a, b], tp) => {
                let va = self.get_value(*a);
                let vb = self.get_value(*b);
                match tp {
                    BinaryType::And => va & vb,
                    BinaryType::Xor => va ^ vb,
                }
            }
            Ternary([a, b, c], tp) => {
                let va = self.get_value(*a);
                let vb = self.get_value(*b);
                let vc = self.get_value(*c);
                match tp {
                    TernaryType::And => va & vb & vc,
                    TernaryType::Xor => va ^ vb ^ vc,
                    TernaryType::Maj => maj(va, vb, vc),
                    TernaryType::Mux => mux(va, vb, vc),
                }
            }
            Dff(_) => self.node_values[i],
            Nary(v, tp) => match tp {
                NaryType::And => self.compute_andn(v, false, false),
                NaryType::Or => self.compute_andn(v, true, true),
                NaryType::Nand => self.compute_andn(v, false, true),
                NaryType::Nor => self.compute_andn(v, true, false),
                NaryType::Xor => self.compute_xorn(v, false),
                NaryType::Xnor => self.compute_xorn(v, true),
            },
            Buf(s) => self.get_value(*s),
            Lut(_) => todo!("Simulation of Lut not implemented"),
        }
    }

    /// Return the result of a single gate with a fault on an input
    pub fn run_gate_with_input_stuck(&self, i: usize, input: usize, value: bool) -> u64 {
        // TODO: this is an ugly duplication but I don't see how to make it cleaner
        assert!(input < self.aig.gate(i).dependencies().len());
        let v = if value { !0u64 } else { 0u64 };
        use crate::Gate::*;
        let g = self.aig.gate(i);
        match g {
            Binary([a, b], tp) => {
                let va = if input == 0 { v } else { self.get_value(*a) };
                let vb = if input == 1 { v } else { self.get_value(*b) };
                match tp {
                    BinaryType::And => va & vb,
                    BinaryType::Xor => va ^ vb,
                }
            }
            Ternary([a, b, c], tp) => {
                let va = if input == 0 { v } else { self.get_value(*a) };
                let vb = if input == 1 { v } else { self.get_value(*b) };
                let vc = if input == 2 { v } else { self.get_value(*c) };
                match tp {
                    TernaryType::And => va & vb & vc,
                    TernaryType::Xor => va ^ vb ^ vc,
                    TernaryType::Maj => maj(va, vb, vc),
                    TernaryType::Mux => mux(va, vb, vc),
                }
            }
            Dff(_) => self.node_values[i],
            Nary(v, tp) => match tp {
                NaryType::And => self.compute_andn_with_input_stuck(v, false, false, input, value),
                NaryType::Or => self.compute_andn_with_input_stuck(v, true, true, input, value),
                NaryType::Nand => self.compute_andn_with_input_stuck(v, false, true, input, value),
                NaryType::Nor => self.compute_andn_with_input_stuck(v, true, false, input, value),
                NaryType::Xor => self.compute_xorn_with_input_stuck(v, false, input, value),
                NaryType::Xnor => self.compute_xorn_with_input_stuck(v, true, input, value),
            },
            Buf(_) => v,
            Lut(_) => todo!("Simulation of Lut not implemented"),
        }
    }

    /// Run the combinatorial part of the design with a list of stuck-at-fault errors
    pub fn run_comb_with_faults(&mut self, faults: &Vec<Fault>) {
        assert!(!Fault::has_duplicate_gate(faults));
        for i in 0..self.aig.nb_nodes() {
            self.node_values[i] = self.run_gate(i);
            for f in faults {
                match f {
                    Fault::OutputStuckAtFault { gate, value } => {
                        if *gate == i {
                            self.node_values[i] = if *value { !0u64 } else { 0u64 };
                        }
                    }
                    Fault::InputStuckAtFault { gate, input, value } => {
                        if *gate == i {
                            self.node_values[i] =
                                self.run_gate_with_input_stuck(*gate, *input, *value);
                        }
                    }
                }
            }
        }
    }

    /// Run the combinatorial part of the design
    pub fn run_comb(&mut self) {
        for i in 0..self.aig.nb_nodes() {
            self.node_values[i] = self.run_gate(i);
        }
    }

    fn compute_andn(&self, v: &[Signal], inv_in: bool, inv_out: bool) -> u64 {
        let mut ret = !0u64;
        for s in v {
            ret &= self.get_value(s ^ inv_in);
        }
        if inv_out {
            !ret
        } else {
            ret
        }
    }

    fn compute_xorn(&self, v: &[Signal], inv_out: bool) -> u64 {
        let mut ret = 0u64;
        for s in v {
            ret ^= self.get_value(*s);
        }
        if inv_out {
            !ret
        } else {
            ret
        }
    }

    fn compute_andn_with_input_stuck(
        &self,
        v: &[Signal],
        inv_in: bool,
        inv_out: bool,
        input: usize,
        value: bool,
    ) -> u64 {
        let val = if value ^ inv_in { !0u64 } else { 0u64 };
        let mut ret = !0u64;
        for (i, s) in v.iter().enumerate() {
            ret &= if i == input {
                val
            } else {
                self.get_value(s ^ inv_in)
            };
        }
        if inv_out {
            !ret
        } else {
            ret
        }
    }

    fn compute_xorn_with_input_stuck(
        &self,
        v: &[Signal],
        inv_out: bool,
        input: usize,
        value: bool,
    ) -> u64 {
        let val = if value { !0u64 } else { 0u64 };
        let mut ret = 0u64;
        for (i, s) in v.iter().enumerate() {
            ret ^= if i == input { val } else { self.get_value(*s) };
        }
        if inv_out {
            !ret
        } else {
            ret
        }
    }

    fn get_output_values(&self) -> Vec<u64> {
        let mut ret = Vec::new();
        for o in 0..self.aig.nb_outputs() {
            ret.push(self.get_value(self.aig.output(o)));
        }
        ret
    }
}
