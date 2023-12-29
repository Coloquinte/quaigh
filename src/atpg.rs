//! Test pattern generation

use kdam::{tqdm, BarExt};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::equiv::{difference, prove};
use crate::sim::{simulate_comb_multi, simulate_comb_multi_with_faults, Fault};
use crate::{Gate, Network, Signal};

/// Expose flip_flops as inputs for ATPG
///
/// The inputs are added after the original inputs. Their order matches the order of the flip flops
pub fn expose_dff(aig: &Network) -> Network {
    let mut ret = Network::new();
    ret.add_inputs(aig.nb_inputs());
    for i in 0..aig.nb_outputs() {
        ret.add_output(aig.output(i));
    }
    for i in 0..aig.nb_nodes() {
        if let Gate::Dff([d, en, res]) = aig.gate(i) {
            let new_input = ret.add_input();
            ret.add(Gate::Buf(new_input));
            ret.add_output(*d);
            if !en.is_constant() {
                ret.add_output(*en);
            }
            if !res.is_constant() {
                ret.add_output(*res);
            }
        } else {
            let g = aig.gate(i).clone();
            ret.add(g);
        }
    }
    ret.check();
    ret
}

/// Analyze which of a set of pattern detect a given fault
fn detects_fault(aig: &Network, pattern: &Vec<u64>, fault: Fault) -> u64 {
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
fn detects_fault_single(aig: &Network, pattern: &Vec<bool>, fault: Fault) -> bool {
    let multi_pattern = pattern
        .iter()
        .map(|b| if *b { !0u64 } else { 0u64 })
        .collect();
    let detection = detects_fault(aig, &multi_pattern, fault);
    assert!(detection == 0u64 || detection == !0u64);
    detection == !0u64
}

/// Find a new test pattern for a specific fault using a SAT solver
///
/// Each gate may be in one of two cases:
///     * in the logic cone after the fault: those need to be duplicated with/without the fault
///     * elsewhere, where they don't need to be duplicated
fn find_pattern_detecting_fault(aig: &Network, fault: Fault) -> Option<Vec<bool>> {
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

    let mut fault_aig = Network::new();
    fault_aig.add_inputs(aig.nb_inputs());
    for i in 0..aig.nb_nodes() {
        if !aig.gate(i).is_comb() {
            continue;
        }
        let g = aig.gate(i).remap(fault_translation);
        fault_aig.add(g);
    }
    for i in 0..aig.nb_outputs() {
        fault_aig.add_output(fault_translation(&aig.output(i)));
    }

    let mut diff = difference(aig, &fault_aig);
    diff.make_canonical();
    diff.cleanup();
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

/// Generate random combinatorial patterns
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
    aig: &'a Network,
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
    pub fn from(aig: &'a Network, seed: u64) -> TestPatternGenerator {
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
    fn get_all_faults(aig: &'a Network) -> Vec<Fault> {
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
                det.push(false);
            } else {
                let d = detects_fault_single(self.aig, &pattern, *f);
                self.detection[i] |= d;
                det.push(d);
            }
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
                det.push(0u64);
            } else {
                let d = detects_fault(self.aig, &pattern, *f);
                self.detection[i] |= d != 0u64;
                det.push(d);
            }
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

    /// Check consistency
    pub fn check(&self) {
        assert_eq!(self.patterns.len(), self.pattern_detections.len());
        for p in &self.patterns {
            assert_eq!(p.len(), self.aig.nb_inputs());
        }
        for p in &self.pattern_detections {
            assert_eq!(p.len(), self.nb_faults());
        }
        assert_eq!(self.detection.len(), self.nb_faults());
    }

    /// Compress the existing patterns to keep as few as possible.
    /// This is a minimum set cover problem.
    /// At the moment we solve it with a simple greedy algorithm,
    /// taking the pattern that detects the most new faults each time.
    pub fn compress_patterns(&mut self) {
        let mut remaining_to_detect = self.nb_detected();

        // Which patterns detect a given fault
        let mut fault_to_patterns = Vec::new();
        for f in 0..self.nb_faults() {
            let mut patterns = Vec::new();
            for p in 0..self.nb_patterns() {
                if self.pattern_detections[p][f] {
                    patterns.push(p);
                }
            }
            fault_to_patterns.push(patterns);
        }

        // Which faults are detected by a given pattern
        let mut pattern_to_faults = Vec::new();
        for p in 0..self.nb_patterns() {
            let mut faults = Vec::new();
            for f in 0..self.nb_faults() {
                if self.pattern_detections[p][f] {
                    faults.push(f);
                }
            }
            pattern_to_faults.push(faults);
        }

        // How many new faults each pattern detects
        let mut nb_detected_by_pattern: Vec<_> =
            pattern_to_faults.iter().map(|v| v.len()).collect();
        assert_eq!(fault_to_patterns.len(), self.nb_faults());
        assert_eq!(pattern_to_faults.len(), self.nb_patterns());

        let mut selected_patterns = Vec::new();

        while remaining_to_detect > 0 {
            // Pick the pattern that detects the most faults
            let best_pattern = nb_detected_by_pattern
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.cmp(b))
                .map(|(index, _)| index)
                .unwrap();
            selected_patterns.push(best_pattern);
            remaining_to_detect -= nb_detected_by_pattern[best_pattern];

            // Remove the faults detected by the pattern from consideration
            assert!(nb_detected_by_pattern[best_pattern] > 0);
            for f in &pattern_to_faults[best_pattern] {
                for p in &fault_to_patterns[*f] {
                    nb_detected_by_pattern[*p] -= 1;
                }
                // So we don't remove a fault twice
                fault_to_patterns[*f].clear();
            }
            assert_eq!(nb_detected_by_pattern[best_pattern], 0);
        }

        let mut new_patterns = Vec::new();
        let mut new_detections = Vec::new();
        for p in selected_patterns {
            new_patterns.push(self.patterns[p].clone());
            new_detections.push(self.pattern_detections[p].clone());
        }
        self.patterns = new_patterns;
        self.pattern_detections = new_detections;
    }
}

/// Generate combinatorial test patterns
///
/// This will generate random test patterns, then try to exercize the remaining faults
/// using a SAT solver. The network needs to be combinatorial.
pub fn generate_comb_test_patterns(aig: &Network, seed: u64) -> Vec<Vec<bool>> {
    assert!(aig.is_comb());
    let mut gen = TestPatternGenerator::from(aig, seed);
    let mut progress = tqdm!(total = gen.nb_faults());
    progress.set_description("Faults processed");
    progress
        .write(format!(
            "Analyzing network with {} inputs, {} outputs and {} possible faults",
            aig.nb_inputs(),
            aig.nb_outputs(),
            gen.nb_faults()
        ))
        .unwrap();
    let mut nb_unsuccesful = 0;
    while nb_unsuccesful < 4 {
        let nb_detected_before = gen.nb_detected();
        gen.add_random_patterns(true);
        if nb_detected_before < gen.nb_detected() {
            nb_unsuccesful = 0;
            progress.update_to(gen.nb_detected()).unwrap();
        } else {
            nb_unsuccesful += 1;
        }
        progress.set_postfix(format!("patterns={}", gen.nb_patterns()));
    }
    progress
        .write(format!(
            "Generated {} random patterns, detecting {}/{} faults",
            gen.nb_patterns(),
            gen.nb_detected(),
            gen.nb_faults()
        ))
        .unwrap();
    let mut unobservable = 0;
    for i in 0..gen.nb_faults() {
        if gen.detection[i] {
            continue;
        }
        let p = find_pattern_detecting_fault(aig, gen.faults[i]);
        if let Some(pattern) = p {
            // TODO: generate new patterns opportunistically by mutating this one
            gen.add_single_pattern(pattern, false);
            progress.update_to(gen.nb_detected()).unwrap();
        } else {
            unobservable += 1;
        }
        progress.set_postfix(format!(
            "patterns={} unobservable={}",
            gen.nb_patterns(),
            unobservable
        ));
    }
    progress
        .write(format!(
            "Generated {} patterns total, detecting {}/{} faults",
            gen.nb_patterns(),
            gen.nb_detected(),
            gen.nb_faults()
        ))
        .unwrap();
    gen.check();
    gen.compress_patterns();
    gen.check();
    progress
        .write(format!(
            "Kept {} patterns, detecting {}/{} faults",
            gen.nb_patterns(),
            gen.nb_detected(),
            gen.nb_faults()
        ))
        .unwrap();
    gen.patterns
}
