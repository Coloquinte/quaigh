//! Simulation of a logic network

mod fault;
mod simple_sim;

use crate::Network;

pub use fault::Fault;

/// Simulate a network over multiple timesteps; return the output values
pub fn simulate(a: &Network, input_values: &Vec<Vec<bool>>) -> Vec<Vec<bool>> {
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

/// Simulate a combinatorial network; return the output values
pub fn simulate_comb(a: &Network, input_values: &Vec<bool>) -> Vec<bool> {
    assert!(a.is_comb());
    let input = vec![input_values.clone()];
    let output = simulate(a, &input);
    output[0].clone()
}

/// Simulate a network over multiple input patterns; return the output values
pub fn simulate_multiple(a: &Network, input_values: &Vec<Vec<Vec<bool>>>) -> Vec<Vec<Vec<bool>>> {
    let mut ret = Vec::new();
    for pattern in input_values {
        ret.push(simulate(a, pattern));
    }
    ret
}

/// Simulate a combinatorial network with 64b inputs; return the output values
pub(crate) fn simulate_comb_multi(a: &Network, input_values: &Vec<u64>) -> Vec<u64> {
    let input = vec![input_values.clone()];
    let output = simulate_multi(a, &input);
    output[0].clone()
}

/// Simulate a network over multiple timesteps with 64b inputs; return the output values
pub(crate) fn simulate_multi(a: &Network, input_values: &Vec<Vec<u64>>) -> Vec<Vec<u64>> {
    use simple_sim::SimpleSimulator;
    let mut sim = SimpleSimulator::from_aig(a);
    sim.run(input_values)
}

/// Simulate a combinatorial network with 64b inputs with faults; return the output values
pub(crate) fn simulate_comb_multi_with_faults(
    a: &Network,
    input_values: &Vec<u64>,
    faults: &Vec<Fault>,
) -> Vec<u64> {
    let input = vec![input_values.clone()];
    let output = simulate_multi_with_faults(a, &input, faults);
    output[0].clone()
}

/// Simulate a network over multiple timesteps with 64b inputs; return the output values
pub(crate) fn simulate_multi_with_faults(
    a: &Network,
    input_values: &Vec<Vec<u64>>,
    faults: &Vec<Fault>,
) -> Vec<Vec<u64>> {
    use simple_sim::SimpleSimulator;
    let mut sim = SimpleSimulator::from_aig(a);
    sim.run_with_faults(input_values, faults)
}

#[cfg(test)]
mod tests {
    use crate::{Gate, NaryType, Network};

    use super::simulate;

    #[test]
    fn test_basic() {
        let mut aig = Network::default();
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
        let mut aig = Network::default();
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
        let mut aig = Network::default();
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
