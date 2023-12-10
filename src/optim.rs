//! Optimization of logic networks

use std::collections::HashMap;
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
    ret.sweep();
    ret.dedup();
    ret
}

/// Datastructure representing the factorization process
struct Factoring {
    gates: Vec<Vec<Signal>>,
}

impl Factoring {
    /// Find which pair of signals is most interesting to factor out
    fn find_best_pair(&self) -> Option<(Signal, Signal)> {
        // TODO: this is linear in the number of pairs, while we should amortize across iterations
        // TODO: the number of pairs is quadratic in the number of signals in a gate, while we could ignore non-duplicates
        let mut count = HashMap::<(Signal, Signal), usize>::new();
        for v in &self.gates {
            for (a, b) in v.iter().tuple_combinations() {
                count.entry((*a, *b)).and_modify(|e| *e += 1).or_insert(1);
            }
        }
        if count.is_empty() {
            None
        } else {
            Some(*count.iter().max_by(|a, b| a.1.cmp(b.1)).unwrap().0)
        }
    }

    /// Replace a pair of signals by a new signal in the datastructure
    fn replace_pair(&mut self, pair: (Signal, Signal), merged: Signal) {
        for v in &mut self.gates {
            if v.contains(&pair.0) && v.contains(&pair.1) {
                v.retain(|s| *s != pair.0 && *s != pair.1);
                v.push(merged);
            }
        }
    }
}

/// Helper function to factor an Aig, to specialize by And/Xor
fn factor_gates<F: Fn(&Gate) -> bool, G: Fn(Signal, Signal) -> Gate>(
    aig: &Network,
    pred: F,
    builder: G,
) -> Network {
    assert!(aig.is_topo_sorted());
    let mut ret = Network::new();
    ret.add_inputs(aig.nb_inputs());

    let mut inds = Vec::new();
    let mut gates = Vec::new();
    for i in 0..aig.nb_nodes() {
        let g = aig.gate(i);
        if pred(g) && g.dependencies().len() > 1 {
            gates.push(g.dependencies());
            inds.push(i);
            // Add a dummy gate to be replaced later
            ret.add(Gate::Buf(Signal::zero()));
        } else {
            ret.add(g.clone());
        }
    }
    for i in 0..aig.nb_outputs() {
        ret.add_output(aig.output(i));
    }

    let mut f = Factoring { gates };
    while let Some((a, b)) = f.find_best_pair() {
        let g = builder(a, b);
        let new_sig = ret.add(g);
        f.replace_pair((a, b), new_sig);
    }

    for (i, g) in zip(inds, f.gates) {
        assert!(g.len() == 1);
        ret.replace(i, Gate::Buf(g[0]));
    }

    ret.topo_sort();
    ret.dedup();
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

#[cfg(test)]
mod tests {
    use crate::optim::flatten_nary;
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
}
