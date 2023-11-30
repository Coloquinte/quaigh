//! Simulation of Aig

use rand::{Rng, SeedableRng};

use crate::{Aig, Signal};

/// Structure for simulation based on a simple representation
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
        let mut ret = Vec::new();
        for v in input_values {
            self.run_dff();
            self.copy_inputs(v.as_slice());
            self.run_comb();
            ret.push(self.get_output_values());
        }
        ret
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
                Andn(v) => self.compute_andn(v),
                Xorn(v) => self.compute_xorn(v),
                Buf(s) => self.get_value(*s),
            };
            self.node_values[i] = val;
        }
    }

    fn compute_andn(&self, v: &[Signal]) -> u64 {
        let mut ret = !0u64;
        for s in v {
            ret &= self.get_value(*s);
        }
        ret
    }

    fn compute_xorn(&self, v: &[Signal]) -> u64 {
        let mut ret = !0u64;
        for s in v {
            ret ^= self.get_value(*s);
        }
        ret
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

/// Generate random patterns with a given number of timesteps
pub fn generate_random_patterns(
    nb_inputs: usize,
    nb_timesteps: usize,
    nb_patterns: usize,
    seed: u64,
) -> Vec<Vec<Vec<bool>>> {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut ret = Vec::new();
    for _ in 0..nb_patterns {
        let mut r1 = Vec::new();
        for _ in 0..nb_timesteps {
            let mut r2 = Vec::new();
            for _ in 0..nb_inputs {
                r2.push(rng.gen());
            }
            r1.push(r2);
        }
        ret.push(r1);
    }
    ret
}

#[cfg(test)]
mod tests {
    use crate::Aig;

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
}
