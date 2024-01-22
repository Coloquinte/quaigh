//! Test pattern generation

use std::iter::zip;

use kdam::{tqdm, BarExt};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::equiv::{difference, prove};
use crate::sim::{detects_faults, detects_faults_multi, Fault};
use crate::{Gate, Network, Signal};

/// Expose flip_flops as inputs for ATPG
///
/// Flip-flop outputs are exposed are primary inputs. Flip-flop inputs, including
/// enable and reset, become primary outputs.
/// The new inputs and outputs are added after the original inputs, and their order
/// matches the order of the flip flops.
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

/// Find a new test pattern for a specific fault using a SAT solver
///
/// Each gate may be in one of two cases:
///     * in the logic cone after the fault: those need to be duplicated with/without the fault
///     * elsewhere, where they don't need to be duplicated
/// To keep things simpler, we create the full network with/without the fault, and let basic
/// deduplication handle the rest.
fn find_pattern_detecting_fault(aig: &Network, fault: Fault) -> Option<Vec<bool>> {
    assert!(aig.is_comb());

    let mut fault_aig = aig.clone();
    match fault {
        Fault::OutputStuckAtFault { gate, value } => {
            fault_aig.replace(gate, Gate::Buf(Signal::from(value)));
        }
        Fault::InputStuckAtFault { gate, input, value } => {
            let g =
                aig.gate(gate).remap_with_ind(
                    |s, i| {
                        if i == input {
                            Signal::from(value)
                        } else {
                            *s
                        }
                    },
                );
            fault_aig.replace(gate, g);
        }
    };

    let mut diff = difference(aig, &fault_aig);
    diff.make_canonical();
    diff.cleanup();
    let ret = prove(&diff);
    if let Some(pattern) = &ret {
        assert_eq!(detects_faults(aig, &pattern, &vec![fault]), vec![true]);
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
    pub fn from(aig: &'a Network, faults: Vec<Fault>, seed: u64) -> TestPatternGenerator {
        assert!(aig.is_topo_sorted());
        let nb_faults = faults.len();
        TestPatternGenerator {
            aig,
            faults: faults,
            patterns: Vec::new(),
            pattern_detections: Vec::new(),
            detection: vec![false; nb_faults],
            rng: SmallRng::seed_from_u64(seed),
        }
    }

    /// Extend a vector of boolean vectors with 64 elements at once
    fn extend_vec(v: &mut Vec<Vec<bool>>, added: Vec<u64>) {
        for i in 0..64 {
            v.push(added.iter().map(|d| (d >> i) & 1 != 0).collect());
        }
    }

    /// Obtain all faults, or only the ones that are not yet detected, and their index
    pub fn get_faults(&self, check_already_detected: bool) -> (Vec<Fault>, Vec<usize>) {
        let mut faults = Vec::new();
        let mut indices = Vec::new();
        for (i, f) in self.faults.iter().enumerate() {
            if check_already_detected || !self.detection[i] {
                faults.push(*f);
                indices.push(i);
            }
        }
        (faults, indices)
    }

    /// Add a single pattern to the current set
    #[allow(dead_code)]
    pub fn add_single_pattern(&mut self, pattern: Vec<bool>, check_already_detected: bool) {
        let (faults, indices) = self.get_faults(check_already_detected);
        let detected = detects_faults(self.aig, &pattern, &faults);
        let mut det = vec![false; self.nb_faults()];
        for (i, d) in zip(indices, detected) {
            self.detection[i] |= d;
            det[i] = d;
        }
        self.patterns.push(pattern);
        self.pattern_detections.push(det);
    }

    /// Add a single pattern and random variations to the current set
    pub fn add_random_patterns_from(&mut self, pattern: Vec<bool>, check_already_detected: bool) {
        let mut patterns = Vec::new();
        let num_rounds = 4; // Generate mostly 0s, with 1/16 values being ones
        for b in pattern {
            let mut val = if b { !0 } else { 0 };
            let mut change = !0;
            for _ in 0..num_rounds {
                change &= self.rng.gen::<u64>();
            }
            val ^= change;
            val &= !1; // Ensure that the first pattern is the original one
            patterns.push(val);
        }
        self.add_patterns(patterns, check_already_detected);
    }

    /// Add a new set of patterns to the current set
    pub fn add_patterns(&mut self, patterns: Vec<u64>, check_already_detected: bool) {
        let (faults, indices) = self.get_faults(check_already_detected);
        let detected = detects_faults_multi(self.aig, &patterns, &faults);
        let mut det = vec![0; self.nb_faults()];
        for (i, d) in zip(indices, detected) {
            self.detection[i] |= d != 0;
            det[i] = d;
        }
        Self::extend_vec(&mut self.patterns, patterns);
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
        let mut progress =
            tqdm!(total = 2 * self.nb_faults() * self.nb_patterns() + self.nb_detected());
        progress.set_description("Compression progress");
        progress
            .set_bar_format("{desc}{percentage:3.0}%|{animation}| [{elapsed}<{remaining}{postfix}]")
            .unwrap();
        progress.set_postfix(format!("patterns=-"));
        let mut remaining_to_detect = self.nb_detected();
        let mut it = 0;

        // Which patterns detect a given fault
        let mut fault_to_patterns = Vec::new();
        for f in 0..self.nb_faults() {
            let mut patterns = Vec::new();
            for p in 0..self.nb_patterns() {
                if self.pattern_detections[p][f] {
                    patterns.push(p);
                }
                it += 1;
                if it % 256 == 0 {
                    progress.update_to(it).unwrap();
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
                it += 1;
                if it % 256 == 0 {
                    progress.update_to(it).unwrap();
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
        progress.update_to(it).unwrap();
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
            progress.set_postfix(format!("patterns={}", selected_patterns.len()));
            progress
                .update(nb_detected_by_pattern[best_pattern])
                .unwrap();

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
        println!();
    }

    pub fn detect_faults(&mut self) {
        let mut progress = tqdm!(total = self.nb_faults());
        progress.set_description("Detection progress");
        progress
            .set_bar_format("{desc}{percentage:3.0}%|{animation}| [{elapsed}<{remaining}{postfix}]")
            .unwrap();
        loop {
            let nb_detected_before = self.nb_detected();
            self.add_random_patterns(true);
            let nb_detected_after = self.nb_detected();
            progress.set_postfix(format!("patterns={}, unobservable=-", self.nb_patterns()));
            progress.update_to(self.nb_detected()).unwrap();
            if nb_detected_after == self.nb_faults() {
                break;
            }
            if ((nb_detected_after - nb_detected_before) as f64) < (0.01 * self.nb_faults() as f64)
            {
                break;
            }
        }
        progress
            .write(format!(
                "Generated {} random patterns, detecting {}/{} faults ({:.2}% coverage)",
                self.nb_patterns(),
                self.nb_detected(),
                self.nb_faults(),
                100.0 * (self.nb_detected() as f64) / (self.nb_faults() as f64)
            ))
            .unwrap();
        let mut unobservable = 0;
        for i in 0..self.nb_faults() {
            if self.detection[i] {
                continue;
            }
            let p = find_pattern_detecting_fault(self.aig, self.faults[i]);
            if let Some(pattern) = p {
                self.add_random_patterns_from(pattern, false);
            } else {
                unobservable += 1;
            }
            progress.set_postfix(format!(
                "patterns={} unobservable={}",
                self.nb_patterns(),
                unobservable
            ));
            progress
                .update_to(self.nb_detected() + unobservable)
                .unwrap();
        }
        progress
            .write(format!(
                "Generated {} patterns total, detecting {}/{} faults ({:.2}% coverage)",
                self.nb_patterns(),
                self.nb_detected(),
                self.nb_faults(),
                100.0 * (self.nb_detected() as f64) / (self.nb_faults() as f64)
            ))
            .unwrap();
        println!();
    }
}

/// Generate combinatorial test patterns
///
/// This will generate random test patterns, then try to exercize the remaining faults
/// using a SAT solver. The network needs to be combinatorial.
pub fn generate_comb_test_patterns(
    aig: &Network,
    seed: u64,
    with_redundant_faults: bool,
) -> Vec<Vec<bool>> {
    assert!(aig.is_comb());
    let faults = Fault::all(aig);
    let unique_faults = Fault::all_unique(aig);

    println!(
        "Analyzing network with {} inputs, {} outputs, {} faults, {} unique faults",
        aig.nb_inputs(),
        aig.nb_outputs(),
        faults.len(),
        unique_faults.len(),
    );

    let mut gen = TestPatternGenerator::from(
        aig,
        if with_redundant_faults {
            faults.clone()
        } else {
            unique_faults.clone()
        },
        seed,
    );
    gen.detect_faults();
    gen.check();
    gen.compress_patterns();
    gen.check();
    println!(
        "Kept {} patterns, detecting {}/{} faults ({:.2}% coverage)",
        gen.nb_patterns(),
        gen.nb_detected(),
        gen.nb_faults(),
        100.0 * (gen.nb_detected() as f64) / (gen.nb_faults() as f64)
    );
    gen.patterns
}

/// Analyze combinatorial test patterns
///
/// This will show the coverage obtained by these test patterns. The network needs to be combinatorial.
pub fn report_comb_test_patterns(
    aig: &Network,
    patterns: Vec<Vec<bool>>,
    with_redundant_faults: bool,
) {
    assert!(aig.is_comb());
    let faults = Fault::all(aig);
    let unique_faults = Fault::all_unique(aig);

    println!(
        "Analyzing network with {} inputs, {} outputs, {} faults, {} unique faults",
        aig.nb_inputs(),
        aig.nb_outputs(),
        faults.len(),
        unique_faults.len(),
    );

    let mut gen = TestPatternGenerator::from(
        aig,
        if with_redundant_faults {
            faults.clone()
        } else {
            unique_faults.clone()
        },
        0,
    );
    for pattern in tqdm!(patterns.iter()) {
        // TODO: make it faster by using multi-pattern simulation
        gen.add_single_pattern(pattern.clone(), false);
    }

    println!(
        "Analyzed {} patterns, detecting {}/{} faults ({:.2}% coverage)",
        gen.nb_patterns(),
        gen.nb_detected(),
        gen.nb_faults(),
        100.0 * (gen.nb_detected() as f64) / (gen.nb_faults() as f64)
    );
}
