//! Simulation of a logic network. Faster, multi-pattern simulation methods are available internally.

mod fault;
mod incremental_sim;
mod simple_sim;

use crate::sim::incremental_sim::IncrementalSimulator;
use crate::Network;

pub use fault::Fault;

/// Simple conversion to 64b format
fn bool_to_multi(values: &Vec<Vec<bool>>) -> Vec<Vec<u64>> {
    let mut ret = Vec::<Vec<u64>>::new();
    for v in values {
        ret.push(v.iter().map(|b| if *b { !0 } else { 0 }).collect());
    }
    ret
}

/// Simple conversion from 64b format
fn multi_to_bool(values: &Vec<Vec<u64>>) -> Vec<Vec<bool>> {
    let mut ret = Vec::new();
    for v in values {
        ret.push(v.iter().map(|b| *b != 0).collect());
    }
    ret
}

/// Simulate a network over multiple timesteps; return the output values
pub fn simulate(a: &Network, input_values: &Vec<Vec<bool>>) -> Vec<Vec<bool>> {
    let multi_input = bool_to_multi(input_values);
    let multi_ret = simulate_multi(a, &multi_input);
    multi_to_bool(&multi_ret)
}

/// Simulate a combinatorial network; return the output values
pub fn simulate_comb(a: &Network, input_values: &Vec<bool>) -> Vec<bool> {
    assert!(a.is_comb());
    let input = vec![input_values.clone()];
    let output = simulate(a, &input);
    output[0].clone()
}

/// Simulate a network over multiple timesteps, with faults injected; return the output values
pub fn simulate_with_faults(
    a: &Network,
    input_values: &Vec<Vec<bool>>,
    faults: &Vec<Fault>,
) -> Vec<Vec<bool>> {
    let multi_input = bool_to_multi(input_values);
    let multi_ret = simulate_multi_with_faults(a, &multi_input, faults);
    multi_to_bool(&multi_ret)
}

/// Simulate a combinatorial network, with faults injected; return the output values
pub fn simulate_comb_with_faults(
    a: &Network,
    input_values: &Vec<bool>,
    faults: &Vec<Fault>,
) -> Vec<bool> {
    assert!(a.is_comb());
    let input = vec![input_values.clone()];
    let output = simulate_with_faults(a, &input, faults);
    output[0].clone()
}

/// Simulate a network over multiple timesteps with 64b inputs; return the output values
pub(crate) fn simulate_multi(a: &Network, input_values: &Vec<Vec<u64>>) -> Vec<Vec<u64>> {
    use simple_sim::SimpleSimulator;
    let mut sim = SimpleSimulator::from_aig(a);
    sim.run(input_values)
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

/// Analyze which of a set of pattern detect a given fault
pub(crate) fn detects_faults_multi(
    aig: &Network,
    pattern: &Vec<u64>,
    faults: &Vec<Fault>,
) -> Vec<u64> {
    assert!(aig.is_comb());
    assert!(aig.is_topo_sorted());
    let mut incr_sim = IncrementalSimulator::from_aig(aig);
    incr_sim.run_initial(pattern);
    let mut detections = Vec::new();
    for f in faults {
        detections.push(incr_sim.detects_fault(*f));
    }
    detections
}

/// Analyze whether a pattern detects a given fault
pub(crate) fn detects_faults(aig: &Network, pattern: &Vec<bool>, faults: &Vec<Fault>) -> Vec<bool> {
    let multi_pattern = pattern
        .iter()
        .map(|b| if *b { !0u64 } else { 0u64 })
        .collect();
    let detections = detects_faults_multi(aig, &multi_pattern, faults);
    detections
        .iter()
        .map(|d| {
            debug_assert!(*d == 0u64 || *d == !0u64);
            *d != 0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use volute::{Lut3, Lut5};

    use crate::network::NaryType;
    use crate::sim::simulate_multi;
    use crate::{Gate, Network, Signal};

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
        let x0 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::And));
        aig.add_output(x0);
        let x1 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Xor));
        aig.add_output(x1);
        let x2 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Or));
        aig.add_output(x2);
        let x3 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Nand));
        aig.add_output(x3);
        let x4 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Nor));
        aig.add_output(x4);
        let x5 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::Xnor));
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

    #[test]
    fn test_maj() {
        let mut aig = Network::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let x0 = aig.add(Gate::maj(i0, i1, i2));
        aig.add_output(x0);
        let pattern = vec![
            vec![false, false, false],
            vec![true, false, false],
            vec![false, true, false],
            vec![false, false, true],
            vec![true, true, true],
        ];
        let expected = vec![
            vec![false],
            vec![false],
            vec![false],
            vec![false],
            vec![true],
        ];
        assert_eq!(simulate(&aig, &pattern), expected);
    }

    #[test]
    fn test_lfsr() {
        let mut aig = Network::default();

        // 3bit LFSR with seed 001, coefficients 101
        // expected steps: 001 100 110 111 011 101 010 001
        // expected outputs: 1 0 0 1 1 1 0 1
        // expected network:
        //
        // x3x2x1 transform to x3'x2'x1' with:
        //
        // x3' = (x3 ^ x1)
        // x2' = x3
        // x1' = x2 | reset
        //
        // and output = x1'

        let reset = aig.add_input();
        let enable = aig.add_input();
        let x3 = aig.dff(
            Signal::placeholder(),
            Signal::placeholder(),
            Signal::placeholder(),
        );
        let x2 = aig.dff(
            Signal::placeholder(),
            Signal::placeholder(),
            Signal::placeholder(),
        );
        let x1 = aig.dff(
            Signal::placeholder(),
            Signal::placeholder(),
            Signal::placeholder(),
        );

        let x3_next = aig.xor(x3, x1);
        let x2_next = x3;
        let x1_next = aig.add(Gate::maj(x2, reset, Signal::one()));

        let enable_on_reset = aig.add(Gate::maj(enable, reset, Signal::one()));

        aig.replace(0, Gate::dff(x3_next, enable, reset));
        aig.replace(1, Gate::dff(x2_next, enable, reset));
        aig.replace(2, Gate::dff(x1_next, enable_on_reset, Signal::zero()));

        aig.add_output(x1);

        let pattern = vec![
            vec![true, true], // step 0 reset to initial state
            vec![false, true],
            vec![false, true],
            vec![false, true],
            vec![false, true],
            vec![false, true],
            vec![false, true],
            vec![false, true],
            vec![false, true],
        ];

        // expected outputs: 0(uinit) 1(inited) 0 0 1 1 1 0 1
        let expected: Vec<Vec<_>> = vec![0, 1, 0, 0, 1, 1, 1, 0, 1]
            .into_iter()
            .map(|b| vec![b == 1])
            .collect();

        assert_eq!(simulate(&aig, &pattern), expected);
    }

    #[test]
    fn test_lut() {
        let mut aig = Network::default();

        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let i3 = aig.add_input();
        let i4 = aig.add_input();
        let truth = Lut5::threshold(4);
        let lut = Gate::lut(&[i0, i1, i2, i3, i4], truth.into());
        let o0 = aig.add(lut);
        aig.add_output(o0);

        let pattern = vec![
            vec![0, 0, 0, 0, 0],
            vec![0b111110, 0b111100, 0b111000, 0b110000, 0b100000],
        ];

        let expected: Vec<Vec<_>> = vec![vec![0], vec![0b110000]];

        assert_eq!(simulate_multi(&aig, &pattern), expected);
    }

    #[test]
    fn test_lut_input_order() {
        let mut aig = Network::default();

        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        for i in 0..3 {
            let o = aig.add(Gate::lut(&[i0, i1, i2], Lut3::nth_var(i).into()));
            aig.add_output(o);
        }

        let pattern = vec![vec![0b1110, 0b1100, 0b1000]];

        let expected: Vec<Vec<_>> = vec![vec![0b1110, 0b1100, 0b1000]];

        assert_eq!(simulate_multi(&aig, &pattern), expected);
    }
}
