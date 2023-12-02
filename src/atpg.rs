//! Test pattern generation

use crate::sim::simulate_comb;
use crate::Aig;

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
        let out = fault_aig.add_raw_gate(g.remap(&t));
        let saf0_in = fault_aig.add_input();
        let saf1_in = fault_aig.add_input();
        let saf0 = fault_aig.and(out, !saf0_in);
        let saf1 = fault_aig.or(saf0, saf1_in);
        t.push(saf1);
    }
    for i in 0..aig.nb_outputs() {
        fault_aig.add_output(aig.output(i).remap(&t));
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
}

/// Report on the given test patterns
pub fn report_test_patterns(aig: &Aig, patterns: &Vec<Vec<bool>>) {
    let analyzer = TestPatternAnalyzer::from(aig);
    println!(
        "{} test patterns, {} possible faults",
        patterns.len(),
        analyzer.nb_faults()
    );
    let detected = analyzer.detected_faults(patterns);
    let number = detected.into_iter().filter(|b| *b).count();
    println!("Detected {} faults out of {}", number, analyzer.nb_faults());
}
