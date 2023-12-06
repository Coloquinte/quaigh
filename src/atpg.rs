//! Test pattern generation

use rand::{Rng, SeedableRng};

use crate::equiv::{difference, prove};
use crate::sim::{simulate_comb, simulate_comb_multi, simulate_comb_multi_with_faults, Fault};
use crate::{Aig, Gate, Signal};

/// Expose flip_flops as inputs for ATPG
///
/// The inputs are added after the original inputs. Their order matches the order of the flip flops
pub fn expose_dff(aig: &Aig) -> Aig {
    let mut ret = Aig::new();
    ret.add_inputs(aig.nb_inputs());
    for i in 0..aig.nb_outputs() {
        ret.add_output(aig.output(i));
    }
    for i in 0..aig.nb_nodes() {
        if let Gate::Dff(d, en, res) = aig.gate(i) {
            let new_input = ret.add_input();
            ret.add_raw_gate(Gate::Buf(new_input));
            ret.add_output(*d);
            if !en.is_constant() {
                ret.add_output(*en);
            }
            if !res.is_constant() {
                ret.add_output(*res);
            }
        } else {
            let g = aig.gate(i).clone();
            ret.add_raw_gate(g);
        }
    }
    ret.check();
    ret
}

/// Analyze which of a set of pattern detect a given fault
pub fn detects_fault(aig: &Aig, pattern: &Vec<u64>, fault: Fault) -> u64 {
    assert!(aig.is_comb());
    assert!(aig.is_topo_sorted());
    let expected = simulate_comb_multi(aig, pattern);
    let obtained = simulate_comb_multi_with_faults(aig, pattern, &vec![fault]);
    let mut detection = 0u64;
    for (a, b) in std::iter::zip(expected, obtained) {
        detection |= a ^ b;
    }
    detection
}

/// Build an Aig with additional inputs to represent error cases
///
/// The resulting Aig will have 2 additional inputs per gate, representing a
/// stuck-at-fault error at the output of the corresponding gate. Keeping these
/// inputs at 0 preserves the behaviour of the network, while setting 1 simulates
/// a stuck-at-fault error.
pub fn build_analysis_network(aig: &Aig) -> Aig {
    assert!(aig.is_comb() && aig.is_topo_sorted());
    let mut fault_aig = Aig::new();
    fault_aig.add_inputs(aig.nb_inputs());

    let mut t = Vec::new();
    for i in 0..aig.nb_nodes() {
        let g = aig.gate(i);
        let out = fault_aig.add_raw_gate(g.remap_order(&t));
        let saf0_in = fault_aig.add_input();
        let saf1_in = fault_aig.add_input();
        let saf0 = fault_aig.and(out, !saf0_in);
        let saf1 = fault_aig.or(saf0, saf1_in);
        t.push(saf1);
    }
    for i in 0..aig.nb_outputs() {
        fault_aig.add_output(aig.output(i).remap_order(&t));
    }

    fault_aig
}

/// Analysis of test patterns
pub struct TestPatternAnalyzer {
    fault_aig: Aig,
    nb_inputs: usize,
    nb_outputs: usize,
    nb_faults: usize,
}

impl TestPatternAnalyzer {
    /// Construct a new analyzer from an Aig
    pub fn from(aig: &Aig) -> TestPatternAnalyzer {
        let fault_aig = build_analysis_network(aig);
        let nb_inputs = aig.nb_inputs();
        let nb_faults = 2 * aig.nb_nodes();
        let nb_outputs = aig.nb_outputs();
        assert_eq!(nb_inputs + nb_faults, fault_aig.nb_inputs());
        TestPatternAnalyzer {
            fault_aig,
            nb_inputs,
            nb_outputs,
            nb_faults,
        }
    }

    /// Return the number of inputs of the analyzed network
    pub fn nb_inputs(&self) -> usize {
        self.nb_inputs
    }

    /// Return the number of inputs of the analyzed network
    pub fn nb_outputs(&self) -> usize {
        self.nb_outputs
    }

    /// Return the number of faults to match
    pub fn nb_faults(&self) -> usize {
        self.nb_faults
    }

    /// Return which faults are detected by a test pattern
    pub fn detected_faults(&self, patterns: &Vec<Vec<bool>>) -> Vec<bool> {
        let mut ret = vec![false; self.nb_faults];

        for pattern in patterns {
            assert_eq!(pattern.len(), self.nb_inputs);
            let mut input = pattern.clone();
            for _ in 0..self.nb_faults {
                input.push(false);
            }
            let expected = simulate_comb(&self.fault_aig, &input);

            for i in 0..self.nb_faults {
                if ret[i] {
                    continue;
                }
                input[self.nb_inputs + i] = true;
                // Simulate the fault and see if the pattern detects it
                let res = simulate_comb(&self.fault_aig, &input);
                ret[i] = res != expected;
                input[self.nb_inputs + i] = false;
            }
        }

        ret
    }

    /// Returns whether a pattern detects a given fault
    pub fn detects_fault(&self, pattern: &Vec<bool>, fault_ind: usize) -> bool {
        assert_eq!(pattern.len(), self.nb_inputs);
        let mut input = pattern.clone();
        for _ in 0..self.nb_faults {
            input.push(false);
        }
        let expected = simulate_comb(&self.fault_aig, &input);

        input[self.nb_inputs + fault_ind] = true;
        // Simulate the fault and see if the pattern detects it
        let res = simulate_comb(&self.fault_aig, &input);
        res != expected
    }
}

/// Find a new test pattern for a specific fault using a SAT solver
///
/// Each gate may be in one of two cases:
///     * in the logic cone after the fault: those need to be duplicated with/without the fault
///     * elsewhere, where they don't need to be duplicated
fn find_pattern_detecting_fault(aig: &Aig, fault: Fault) -> Option<Vec<bool>> {
    // Translation of the original signal into the fault
    let fault_translation = |s: &Signal| -> Signal {
        match fault {
            Fault::OutputStuckAtFault { gate, value } => {
                if s.is_var() && s.var() == gate as u32 {
                    Signal::from(value) ^ s.is_inverted()
                } else {
                    *s
                }
            }
            _ => todo!(),
        }
    };

    let mut fault_aig = Aig::new();
    fault_aig.add_inputs(aig.nb_inputs());
    for i in 0..aig.nb_nodes() {
        if !aig.gate(i).is_comb() {
            continue;
        }
        let g = aig.gate(i).remap(fault_translation);
        fault_aig.add_raw_gate(g);
    }
    for i in 0..aig.nb_outputs() {
        fault_aig.add_output(fault_translation(&aig.output(i)));
    }

    let mut diff = difference(aig, &fault_aig);
    diff.dedup();
    diff.sweep();
    prove(&diff)
}

/// Generate random patterns with a given number of timesteps
pub fn generate_random_seq_patterns(
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

/// Generate random patterns
pub fn generate_random_comb_patterns(
    nb_inputs: usize,
    nb_patterns: usize,
    seed: u64,
) -> Vec<Vec<bool>> {
    let seq_patterns = generate_random_seq_patterns(nb_inputs, 1, nb_patterns, seed);
    seq_patterns.iter().map(|p| p[0].clone()).collect()
}

/// Generate test patterns
pub fn generate_test_patterns(aig: &Aig, seed: u64) -> Vec<Vec<bool>> {
    assert!(aig.is_comb());
    let mut patterns =
        generate_random_comb_patterns(aig.nb_inputs(), 4 * aig.nb_outputs() + 4, seed);
    let analyzer = TestPatternAnalyzer::from(aig);
    let mut detected_faults = analyzer.detected_faults(&patterns);
    let nb_detected_random = detected_faults.iter().filter(|b| **b).count();
    println!(
        "Generated {} random patterns, detecting {}/{} faults",
        patterns.len(),
        nb_detected_random,
        analyzer.nb_faults()
    );
    for i in 0..analyzer.nb_faults() {
        if detected_faults[i] {
            continue;
        }
        let fault = Fault::OutputStuckAtFault {
            gate: i / 2,
            value: i % 2 != 0,
        };
        let new_pattern = find_pattern_detecting_fault(aig, fault);
        if let Some(p) = new_pattern {
            assert!(analyzer.detects_fault(&p, i));
            detected_faults[i] = true;
            for j in i + 1..analyzer.nb_faults() {
                if detected_faults[j] {
                    continue;
                }
                let new_detection = analyzer.detects_fault(&p, j);
                if new_detection {
                    detected_faults[j] = true;
                }
            }
            patterns.push(p);
        }
    }

    let nb_detected_sat = detected_faults.iter().filter(|b| **b).count();
    println!(
        "Generated {} patterns total, detecting {}/{} faults",
        patterns.len(),
        nb_detected_sat,
        analyzer.nb_faults()
    );
    if nb_detected_sat != analyzer.nb_faults() {
        println!("Not all faults are detectable!");
    }
    patterns
}
