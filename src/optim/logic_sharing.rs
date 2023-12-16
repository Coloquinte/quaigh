//! Logic sharing in logic networks, acting on N-input And and Xor gates

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::iter::zip;

use itertools::Itertools;

use crate::{Gate, NaryType, Network, Signal};

/// Helper functions to merge N-input gates, to specialize by And/Xor
fn merge_dependencies<F: Fn(&Gate) -> bool>(
    aig: &Network,
    g: &Gate,
    max_size: usize,
    pred: F,
) -> Box<[Signal]> {
    let v = g.dependencies();
    let mut ret = Vec::new();
    let mut remaining = v.len();
    for s in v.iter() {
        remaining -= 1;
        if !s.is_var() || s.is_inverted() {
            ret.push(*s);
        } else {
            let prev_g = aig.gate(s.var() as usize);
            let prev_deps = prev_g.dependencies();
            if pred(prev_g) && ret.len() + prev_deps.len() + remaining <= max_size {
                ret.extend(prev_deps);
            } else {
                ret.push(*s);
            }
        }
    }
    ret.into()
}

/// Completely flatten And and Xor gates in a network
///
/// Gates will be completely merged. This can result in very large And and Xor gates which share many inputs.
/// To avoid quadratic blowup, a maximum size can be specified. Gates that do not share inputs will be
/// flattened regardless of their size.
pub fn flatten_nary(aig: &Network, max_size: usize) -> Network {
    let mut ret = Network::new();
    ret.add_inputs(aig.nb_inputs());
    for i in 0..aig.nb_nodes() {
        let g = aig.gate(i);
        let merged = if g.is_and() {
            Gate::Nary(
                merge_dependencies(&ret, g, max_size, |t| t.is_and()),
                NaryType::And,
            )
        } else if g.is_xor() {
            Gate::Nary(
                merge_dependencies(&ret, g, max_size, |t| t.is_xor()),
                NaryType::Xor,
            )
        } else {
            g.clone()
        };
        ret.add(merged);
    }
    for i in 0..aig.nb_outputs() {
        ret.add_output(aig.output(i));
    }
    ret.cleanup();
    ret.make_canonical();
    ret
}

/// Datastructure representing the factorization process
struct Factoring {
    /// Gates left to factor
    gates: Vec<Vec<Signal>>,
    /// Next variable index to be allocated
    next_var: u32,
    /// Pairs that have already been built
    built_pairs: Vec<(Signal, Signal)>,
    /// Pairs organized by bucket of usage count
    count_to_pair: Vec<HashSet<(Signal, Signal)>>,
    /// Pairs to their usage location
    pair_to_gates: HashMap<(Signal, Signal), HashSet<usize>>,
    // TODO: use faster hashmaps
    // TODO: handle the usual case (no sharing) separately
}

impl Factoring {
    /// Build from the list of gates
    fn from_gates(gates: Vec<Vec<Signal>>, next_var: u32) -> Factoring {
        Factoring {
            gates,
            next_var,
            built_pairs: Vec::new(),
            count_to_pair: Vec::new(),
            pair_to_gates: HashMap::new(),
        }
    }

    /// Create a pair from two signals
    fn make_pair(a: &Signal, b: &Signal) -> (Signal, Signal) {
        (cmp::min(*a, *b), cmp::max(*a, *b))
    }

    /// Count the number of time each signal is used
    fn count_signal_usage(&self) -> HashMap<Signal, u32> {
        let mut count = HashMap::<Signal, u32>::new();
        for v in &self.gates {
            for s in v {
                count.entry(*s).and_modify(|e| *e += 1).or_insert(1);
            }
        }
        count
    }

    /// Gather the gates where each pair is used
    fn compute_pair_to_gates(&self) -> HashMap<(Signal, Signal), HashSet<usize>> {
        let mut ret = HashMap::<(Signal, Signal), HashSet<usize>>::new();
        for (i, v) in self.gates.iter().enumerate() {
            for (a, b) in v.iter().tuple_combinations() {
                let p = Factoring::make_pair(a, b);
                ret.entry(p)
                    .and_modify(|e| {
                        e.insert(i);
                    })
                    .or_insert(HashSet::from([i]));
            }
        }
        ret
    }

    /// Setup the datastructures
    fn setup_initial(&mut self) {
        self.pair_to_gates = self.compute_pair_to_gates();
        for (p, gates_touched) in &self.pair_to_gates {
            let cnt = gates_touched.len();
            if self.count_to_pair.len() <= cnt {
                self.count_to_pair.resize(cnt + 1, HashSet::new());
            }
            self.count_to_pair[cnt].insert(*p);
        }
    }

    /// Remove one pair from everywhere it is used
    fn replace_pair(&mut self, p: (Signal, Signal)) {
        let p_out = self.new_var();
        self.built_pairs.push(p);
        let gates_touched = self.pair_to_gates.remove(&p).unwrap();
        self.count_to_pair[gates_touched.len()].remove(&p);
        for i in gates_touched {
            self.gates[i].retain(|s| *s != p.0 && *s != p.1);
            for s in self.gates[i].clone() {
                self.decrement_pair(Factoring::make_pair(&s, &p.0), i);
                self.decrement_pair(Factoring::make_pair(&s, &p.1), i);
                self.increment_pair(Factoring::make_pair(&s, &p_out), i);
                self.increment_pair(Factoring::make_pair(&s, &p_out), i);
            }
            self.gates[i].push(p_out);
        }
    }

    /// Decrement the usage of one pair
    fn decrement_pair(&mut self, p: (Signal, Signal), gate: usize) {
        let cnt = self.pair_to_gates[&p].len();
        self.pair_to_gates.entry(p).and_modify(|e| {
            e.remove(&gate);
        });
        self.count_to_pair[cnt].remove(&p);
        if cnt > 1 {
            self.count_to_pair[cnt - 1].insert(p);
        }
    }

    /// Increment the usage of one pair
    fn increment_pair(&mut self, p: (Signal, Signal), gate: usize) {
        self.pair_to_gates
            .entry(p)
            .and_modify(|e| {
                e.insert(gate);
            })
            .or_insert(HashSet::from([gate]));
        let cnt = self.pair_to_gates[&p].len();
        if self.count_to_pair.len() <= cnt {
            self.count_to_pair.resize(cnt + 1, HashSet::new());
        }
        self.count_to_pair[cnt - 1].remove(&p);
        self.count_to_pair[cnt].insert(p);
    }

    /// Allocate the next var to be used in a pair
    fn new_var(&mut self) -> Signal {
        let ret = Signal::from_var(self.next_var);
        self.next_var += 1;
        ret
    }

    /// Find the pair to add
    fn find_best_pair(&mut self) -> Option<(Signal, Signal)> {
        while !self.count_to_pair.is_empty() {
            let pairs = self.count_to_pair.last().unwrap();
            if let Some(p) = pairs.iter().next() {
                return Some(*p);
            } else {
                self.count_to_pair.pop();
            }
        }
        None
    }

    /// Share logic between the pairs
    fn consume_pairs(&mut self) {
        self.setup_initial();
        while let Some(p) = self.find_best_pair() {
            self.replace_pair(p);
        }
    }

    /// Run factoring of the gates, and return the resulting binary gates to create
    pub fn run(gates: Vec<Vec<Signal>>, first_var: u32) -> (Vec<(Signal, Signal)>, Vec<Signal>) {
        let mut f = Factoring::from_gates(gates, first_var);
        f.consume_pairs();
        for g in &f.gates {
            assert!(g.len() == 1);
        }
        let replacement = f.gates.iter().map(|g| g[0]).collect();
        (f.built_pairs, replacement)
    }
}

/// Helper function to factor an Aig, to specialize by And/Xor
fn factor_gates<F: Fn(&Gate) -> bool, G: Fn(Signal, Signal) -> Gate>(
    aig: &Network,
    pred: F,
    builder: G,
) -> Network {
    assert!(aig.is_topo_sorted());

    let mut inds = Vec::new();
    let mut gates = Vec::new();
    for i in 0..aig.nb_nodes() {
        let g = aig.gate(i);
        if pred(g) && g.dependencies().len() > 1 {
            gates.push(g.dependencies());
            inds.push(i);
        }
    }

    let mut ret = aig.clone();
    let (binary_gates, replacements) = Factoring::run(gates, ret.nb_nodes() as u32);
    for (a, b) in binary_gates {
        ret.add(builder(a, b));
    }

    for (i, g) in zip(inds, replacements) {
        ret.replace(i, Gate::Buf(g));
    }

    // Necessary to cleanup as we have gates
    ret.topo_sort();
    ret.make_canonical();
    ret
}

/// Factor And or Xor gates with common inputs
///
/// Transform large gates into trees of binary gates, sharing as many inputs as possible.
/// The optimization is performed greedily by merging the most used pair of inputs at each step.
/// There is no delay optimization yet.
pub fn factor_nary(aig: &Network) -> Network {
    let aig1 = factor_gates(aig, |g| g.is_and(), |a, b| Gate::And(a, b));
    let aig2 = factor_gates(&aig1, |g| g.is_xor(), |a, b| Gate::Xor(a, b));
    aig2
}

/// Share logic between N-ary gates
///
/// Reorganizes logic into N-input gates, then creates trees of 2-input gates that share as much logic as possible
pub fn share_logic(aig: &mut Network, flattening_limit: usize) {
    *aig = flatten_nary(&aig, flattening_limit);
    *aig = factor_nary(&aig);
}

#[cfg(test)]
mod tests {
    use super::{factor_nary, flatten_nary};
    use crate::{Gate, NaryType, Network, Signal};

    #[test]
    fn test_flatten_and() {
        let mut aig = Network::new();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        aig.add_input();
        let i4 = aig.add_input();
        let x0 = aig.and(i0, i1);
        let x1 = aig.and(i0, !i2);
        let x2 = aig.and(x0, x1);
        let x3 = aig.and(x2, i4);
        aig.add_output(x3);
        aig = flatten_nary(&aig, 64);
        assert_eq!(aig.nb_nodes(), 1);
        assert_eq!(
            aig.gate(0),
            &Gate::Nary(Box::new([i4, !i2, i1, i0]), NaryType::And)
        );
    }

    #[test]
    fn test_flatten_xor() {
        let mut aig = Network::new();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        aig.add_input();
        let i4 = aig.add_input();
        let x0 = aig.xor(i0, i1);
        let x1 = aig.xor(i0, !i2);
        let x2 = aig.xor(x0, x1);
        let x3 = aig.xor(x2, i4);
        aig.add_output(x3);
        aig = flatten_nary(&aig, 64);
        assert_eq!(aig.nb_nodes(), 1);
        assert_eq!(aig.gate(0), &Gate::Xor3(i4, i2, i1));
        assert_eq!(aig.output(0), !Signal::from_var(0));
    }

    #[test]
    fn test_share_and() {
        let mut aig = Network::new();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let i3 = aig.add_input();
        let i4 = aig.add_input();
        let x0 = aig.add(Gate::Nary(Box::new([i0, i1, i2]), NaryType::And));
        let x1 = aig.add(Gate::Nary(Box::new([i0, i1, i2, i3]), NaryType::And));
        let x2 = aig.add(Gate::Nary(Box::new([i1, i2, i4]), NaryType::And));
        aig.add_output(x0);
        aig.add_output(x1);
        aig.add_output(x2);
        aig = factor_nary(&aig);
        assert_eq!(aig.nb_nodes(), 4);
        // Check that the first gate is the most shared
        assert_eq!(aig.gate(0), &Gate::And(i2, i1));
    }
}
