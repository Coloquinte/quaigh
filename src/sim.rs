//! Simulation of a logic network

use crate::{Aig, NaryType, Signal};

/// Structure for simulation based directly on the network representation
struct SimpleSimulator<'a> {
    aig: &'a Aig,
    input_values: Vec<u64>,
    node_values: Vec<u64>,
}

fn maj(a: u64, b: u64, c: u64) -> u64 {
    (b & c) | (a & (b | c))
}

fn mux(s: u64, a: u64, b: u64) -> u64 {
    (s & a) | (!s & b)
}

impl<'a> SimpleSimulator<'a> {
    fn from_aig(aig: &'a Aig) -> SimpleSimulator<'a> {
        assert!(aig.is_topo_sorted());
        SimpleSimulator {
            aig,
            input_values: vec![0; aig.nb_inputs()],
            node_values: vec![0; aig.nb_nodes()],
        }
    }

    fn run(&mut self, input_values: &Vec<Vec<u64>>) -> Vec<Vec<u64>> {
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
            self.input_values[s.input() as usize] ^ s.pol_to_word()
        } else {
            debug_assert!(s.is_var());
            self.node_values[s.var() as usize] ^ s.pol_to_word()
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

/// Simulate an Aig over multiple timesteps; return the output values
pub fn simulate(a: &Aig, input_values: &Vec<Vec<bool>>) -> Vec<Vec<bool>> {
    let mut multi_input = Vec::<Vec<u64>>::new();
    for v in input_values {
        multi_input.push(v.iter().map(|b| if *b { !0 } else { 0 }).collect());
    }
    let multi_ret = simulate_multi(a, &multi_input);
    let mut ret = Vec::new();
    for v in multi_ret {
        ret.push(v.iter().map(|b| *b != 0).collect());
    }
    ret
}

/// Simulate a combinatorial Aig; return the output values
pub fn simulate_comb(a: &Aig, input_values: &Vec<bool>) -> Vec<bool> {
    assert!(a.is_comb());
    let input = vec![input_values.clone()];
    let output = simulate(a, &input);
    output[0].clone()
}

/// Simulate an Aig over multiple input patterns; return the output values
pub fn simulate_multiple(a: &Aig, input_values: &Vec<Vec<Vec<bool>>>) -> Vec<Vec<Vec<bool>>> {
    let mut ret = Vec::new();
    for pattern in input_values {
        ret.push(simulate(a, pattern));
    }
    ret
}

/// Simulate an Aig over multiple timesteps with 64b inputs; return the output values
fn simulate_multi(a: &Aig, input_values: &Vec<Vec<u64>>) -> Vec<Vec<u64>> {
    let mut sim = SimpleSimulator::from_aig(a);
    sim.run(input_values)
}

#[cfg(test)]
mod tests {
    use crate::{Aig, Gate, NaryType};

    use super::simulate;

    #[test]
    fn test_basic() {
        let mut aig = Aig::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let x1 = aig.xor(i0, i1);
        let x2 = aig.and(i0, i2);
        let x3 = aig.and(x2, !i1);
        aig.add_output(x1);
        aig.add_output(x3);

        assert_eq!(
            simulate(&aig, &vec![vec![false, false, false]]),
            vec![vec![false, false]]
        );
        assert_eq!(
            simulate(&aig, &vec![vec![true, false, false]]),
            vec![vec![true, false]]
        );
        assert_eq!(
            simulate(&aig, &vec![vec![true, false, true]]),
            vec![vec![true, true]]
        );
        assert_eq!(
            simulate(&aig, &vec![vec![true, true, true]]),
            vec![vec![false, false]]
        );
    }

    #[test]
    fn test_dff() {
        let mut aig = Aig::default();
        let d = aig.add_input();
        let en = aig.add_input();
        let res = aig.add_input();
        let x = aig.dff(d, en, res);
        aig.add_output(x);
        let pattern = vec![
            vec![false, false, false],
            vec![false, true, false],
            vec![true, true, false],
            vec![true, false, false],
            vec![true, false, true],
            vec![false, false, false],
        ];
        let expected = vec![
            vec![false],
            vec![false],
            vec![false],
            vec![true],
            vec![true],
            vec![false],
        ];
        assert_eq!(simulate(&aig, &pattern), expected);
    }

    #[test]
    fn test_nary() {
        let mut aig = Aig::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let i3 = aig.add_input();
        let x0 = aig.add_raw_gate(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::And));
        aig.add_output(x0);
        let x1 = aig.add_raw_gate(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Xor));
        aig.add_output(x1);
        let x2 = aig.add_raw_gate(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Or));
        aig.add_output(x2);
        let x3 = aig.add_raw_gate(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Nand));
        aig.add_output(x3);
        let x4 = aig.add_raw_gate(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Nor));
        aig.add_output(x4);
        let x5 = aig.add_raw_gate(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Xnor));
        aig.add_output(x5);

        let pattern = vec![
            vec![false, false, false, false],
            vec![true, false, false, false],
            vec![false, true, false, false],
            vec![false, false, true, false],
            vec![false, false, false, true],
            vec![true, true, true, true],
        ];
        let expected = vec![
            vec![false, false, false, true, true, true],
            vec![false, true, true, true, false, false],
            vec![false, true, true, true, false, false],
            vec![false, true, true, true, false, false],
            vec![false, true, true, true, false, false],
            vec![true, false, true, false, false, true],
        ];
        assert_eq!(simulate(&aig, &pattern), expected);
    }
}
