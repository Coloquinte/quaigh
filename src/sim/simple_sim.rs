use crate::{Aig, NaryType, Signal};

/// Structure for simulation based directly on the network representation
pub struct SimpleSimulator<'a> {
    aig: &'a Aig,
    input_values: Vec<u64>,
    node_values: Vec<u64>,
}

/// Convert the inversion to a word for bitwise operations
fn pol_to_word(s: Signal) -> u64 {
    let pol = s.raw() & 1;
    (!(pol as u64)).wrapping_add(1)
}

fn maj(a: u64, b: u64, c: u64) -> u64 {
    (b & c) | (a & (b | c))
}

fn mux(s: u64, a: u64, b: u64) -> u64 {
    (s & a) | (!s & b)
}

impl<'a> SimpleSimulator<'a> {
    pub fn from_aig(aig: &'a Aig) -> SimpleSimulator<'a> {
        assert!(aig.is_topo_sorted());
        SimpleSimulator {
            aig,
            input_values: vec![0; aig.nb_inputs()],
            node_values: vec![0; aig.nb_nodes()],
        }
    }

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

    fn reset(&mut self) {
        self.input_values = vec![0; self.aig.nb_inputs()];
        self.node_values = vec![0; self.aig.nb_nodes()];
    }

    fn check(&self) {
        assert_eq!(self.input_values.len(), self.aig.nb_inputs());
        assert_eq!(self.node_values.len(), self.aig.nb_nodes());
    }

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

    fn copy_inputs(&mut self, inputs: &[u64]) {
        assert_eq!(inputs.len(), self.input_values.len());
        self.input_values.copy_from_slice(inputs);
    }

    fn run_dff(&mut self) {
        use crate::Gate::*;
        let mut next_values = self.node_values.clone();
        for i in 0..self.aig.nb_nodes() {
            let g = self.aig.gate(i);
            if let Dff(d, en, res) = g {
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

    fn run_comb(&mut self) {
        use crate::Gate::*;
        for i in 0..self.aig.nb_nodes() {
            let g = self.aig.gate(i);
            let val = match g {
                And(a, b) => self.get_value(*a) & self.get_value(*b),
                Xor(a, b) => self.get_value(*a) ^ self.get_value(*b),
                And3(a, b, c) => self.get_value(*a) & self.get_value(*b) & self.get_value(*c),
                Xor3(a, b, c) => self.get_value(*a) ^ self.get_value(*b) ^ self.get_value(*c),
                Maj(a, b, c) => maj(self.get_value(*a), self.get_value(*b), self.get_value(*c)),
                Mux(a, b, c) => mux(self.get_value(*a), self.get_value(*b), self.get_value(*c)),
                Dff(_, _, _) => continue,
                Nary(v, tp) => match tp {
                    NaryType::And => self.compute_andn(v, false, false),
                    NaryType::Or => self.compute_andn(v, true, true),
                    NaryType::Nand => self.compute_andn(v, false, true),
                    NaryType::Nor => self.compute_andn(v, true, false),
                    NaryType::Xor => self.compute_xorn(v, false),
                    NaryType::Xnor => self.compute_xorn(v, true),
                },
                Buf(s) => self.get_value(*s),
            };
            self.node_values[i] = val;
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

    fn get_output_values(&self) -> Vec<u64> {
        let mut ret = Vec::new();
        for o in 0..self.aig.nb_outputs() {
            ret.push(self.get_value(self.aig.output(o)));
        }
        ret
    }
}
