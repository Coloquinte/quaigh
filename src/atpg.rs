//! Test pattern generation

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::equiv::{difference, prove};
use crate::sim::{simulate_comb_multi, simulate_comb_multi_with_faults, Fault};
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
fn detects_fault(aig: &Aig, pattern: &Vec<u64>, fault: Fault) -> u64 {
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

/// Analyze whether a pattern detects a given fault
fn detects_fault_single(aig: &Aig, pattern: &Vec<bool>, fault: Fault) -> bool {
    let multi_pattern = pattern
        .iter()
        .map(|b| if *b { !0u64 } else { 0u64 })
        .collect();
    let detection = detects_fault(aig, &multi_pattern, fault);
    assert!(detection == 0u64 || detection == !0u64);
    detection == !0u64
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
    let ret = prove(&diff);
    if let Some(pattern) = &ret {
        assert!(detects_fault_single(aig, &pattern, fault));
    }
    ret
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

/// Handling of the actual test pattern generation
struct TestPatternGenerator<'a> {
    aig: &'a Aig,
    faults: Vec<Fault>,
    patterns: Vec<Vec<bool>>,
    pattern_detections: Vec<Vec<bool>>,
    detection: Vec<bool>,
    rng: SmallRng,
}

impl<'a> TestPatternGenerator<'a> {
    pub fn nb_faults(&self) -> usize {
        self.faults.len()
    }

    pub fn nb_patterns(&self) -> usize {
        self.patterns.len()
    }

    pub fn nb_detected(&self) -> usize {
        self.detection.iter().filter(|b| **b).count()
    }

    /// Initialize the generator from a network and a seed
    pub fn from(aig: &'a Aig, seed: u64) -> TestPatternGenerator {
        assert!(aig.is_topo_sorted());
        let faults = Self::get_all_faults(aig);
        let nb_faults = faults.len();
        TestPatternGenerator {
            aig,
            faults,
            patterns: Vec::new(),
            pattern_detections: Vec::new(),
            detection: vec![false; nb_faults],
            rng: SmallRng::seed_from_u64(seed),
        }
    }

    /// Get all possible faults in a network
    fn get_all_faults(aig: &'a Aig) -> Vec<Fault> {
        let mut ret = Vec::new();
        for i in 0..aig.nb_nodes() {
            ret.push(Fault::OutputStuckAtFault {
                gate: i,
                value: false,
            });
            ret.push(Fault::OutputStuckAtFault {
                gate: i,
                value: true,
            });
        }
        ret
    }

    /// Extend a vector of boolean vectors with 64 elements at once
    fn extend_vec(v: &mut Vec<Vec<bool>>, added: Vec<u64>) {
        for i in 0..64 {
            v.push(added.iter().map(|d| (d >> i) & 1 != 0).collect());
        }
    }

    /// Add a new set of patterns to the current set
    pub fn add_single_pattern(&mut self, pattern: Vec<bool>, check_already_detected: bool) {
        let mut det = Vec::new();
        for (i, f) in self.faults.iter().enumerate() {
            if !check_already_detected && self.detection[i] {
                continue;
            }
            let d = detects_fault_single(self.aig, &pattern, *f);
            self.detection[i] |= d;
            det.push(d);
        }
        self.patterns.push(pattern);
        self.pattern_detections.push(det);
    }

    /// Add a new set of patterns to the current set
    pub fn add_patterns(&mut self, pattern: Vec<u64>, check_already_detected: bool) {
        assert!(pattern.len() == self.aig.nb_inputs());
        let mut det = Vec::new();
        for (i, f) in self.faults.iter().enumerate() {
            if !check_already_detected && self.detection[i] {
                continue;
            }
            let d = detects_fault(self.aig, &pattern, *f);
            self.detection[i] |= d != 0u64;
            det.push(d);
        }
        Self::extend_vec(&mut self.patterns, pattern);
        Self::extend_vec(&mut self.pattern_detections, det);
    }

    /// Generate a random pattern and add it to the current set
    pub fn add_random_patterns(&mut self, check_already_detected: bool) {
        let pattern = (0..self.aig.nb_inputs())
            .map(|_| self.rng.gen::<u64>())
            .collect();
        self.add_patterns(pattern, check_already_detected);
    }
}

/// Generate test patterns
pub fn generate_test_patterns(aig: &Aig, seed: u64) -> Vec<Vec<bool>> {
    assert!(aig.is_comb());
    let mut gen = TestPatternGenerator::from(aig, seed);
    println!(
        "Analyzing network with {} inputs, {} outputs and {} possible faults",
        aig.nb_inputs(),
        aig.nb_outputs(),
        gen.nb_faults()
    );
    let mut nb_unsuccesful = 0;
    while nb_unsuccesful < 4 {
        let nb_detected_before = gen.nb_detected();
        gen.add_random_patterns(true);
        if nb_detected_before < gen.nb_detected() {
            nb_unsuccesful = 0;
        } else {
            nb_unsuccesful += 1;
        }
    }
    println!(
        "Generated {} random patterns, detecting {}/{} faults",
        gen.nb_patterns(),
        gen.nb_detected(),
        gen.nb_faults()
    );
    for i in 0..gen.nb_faults() {
        if gen.detection[i] {
            continue;
        }
        let p = find_pattern_detecting_fault(aig, gen.faults[i]);
        if let Some(pattern) = p {
            gen.add_single_pattern(pattern, false);
        }
    }
    println!(
        "Generated {} patterns total, detecting {}/{} faults",
        gen.nb_patterns(),
        gen.nb_detected(),
        gen.nb_faults()
    );
    gen.patterns
}
