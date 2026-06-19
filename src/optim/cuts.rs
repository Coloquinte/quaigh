//! Cut enumeration
//!
//! A *cut* of a node is a set of signals (the *leaves*) such that every path from a
//! primary input to the node passes through a leaf: the node's value is fully
//! determined by the leaves. A *k-feasible* cut has at most `k` leaves. Cut enumeration
//! computes, for every node, a set of k-feasible cuts; it is the basis for FPGA
//! technology mapping, local rewriting and many other optimizations.
//!
//! The enumeration is bottom-up: a node's cuts are its trivial self-cut together with
//! every combination of its fanins' cuts whose union stays within `k` leaves. Dominated
//! cuts (supersets of another cut) are removed, and the number of cuts per node is
//! capped to keep high-fanin gates tractable (priority cuts).
//!
//! Flip-flops are treated as combinational boundaries: a flip-flop has only its trivial
//! cut, so cuts never cross a register. Primary inputs are implicit leaves with a single
//! trivial cut and are not part of the returned per-node vector.

use std::fmt;

use crate::{Network, Signal};

/// Default maximum number of cuts kept per node (priority-cut limit)
const DEFAULT_MAX_CUTS: usize = 8;

/// A cut: the set of leaf signals whose values determine the cut's root
///
/// Leaves are stored sorted and without inversion (a cut describes structural support,
/// not polarity). Constants are never leaves.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cut {
    leaves: Vec<Signal>,
}

impl Cut {
    /// The trivial cut of a signal: the signal itself as the only leaf
    fn trivial(s: Signal) -> Cut {
        Cut {
            leaves: vec![s.without_inversion()],
        }
    }

    /// The empty cut (no leaves), used as the identity when merging fanins
    fn empty() -> Cut {
        Cut { leaves: Vec::new() }
    }

    /// The leaves of the cut, sorted and without inversion
    pub fn leaves(&self) -> &[Signal] {
        &self.leaves
    }

    /// Number of leaves
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Whether the cut has no leaves
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// Union of two cuts, or `None` if the result would exceed `max` leaves
    fn union(&self, other: &Cut, max: usize) -> Option<Cut> {
        let mut leaves = Vec::with_capacity(self.leaves.len() + other.leaves.len());
        let (mut i, mut j) = (0, 0);
        while i < self.leaves.len() && j < other.leaves.len() {
            let a = self.leaves[i];
            let b = other.leaves[j];
            if a < b {
                leaves.push(a);
                i += 1;
            } else if b < a {
                leaves.push(b);
                j += 1;
            } else {
                leaves.push(a);
                i += 1;
                j += 1;
            }
            if leaves.len() > max {
                return None;
            }
        }
        if self.leaves.len() - i + leaves.len() > max || other.leaves.len() - j + leaves.len() > max
        {
            return None;
        }
        leaves.extend_from_slice(&self.leaves[i..]);
        leaves.extend_from_slice(&other.leaves[j..]);
        Some(Cut { leaves })
    }

    /// Whether `self` is a subset of `other` (so `self` dominates `other`)
    fn dominates(&self, other: &Cut) -> bool {
        if self.leaves.len() > other.leaves.len() {
            return false;
        }
        let mut j = 0;
        for s in &self.leaves {
            while j < other.leaves.len() && other.leaves[j] < *s {
                j += 1;
            }
            if j >= other.leaves.len() || other.leaves[j] != *s {
                return false;
            }
            j += 1;
        }
        true
    }
}

impl fmt::Display for Cut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (i, s) in self.leaves.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{s}")?;
        }
        write!(f, "}}")
    }
}

/// Remove duplicate and dominated cuts (a cut that is a superset of another)
fn prune_dominated(cuts: &mut Vec<Cut>) {
    cuts.sort_by(|a, b| a.leaves.cmp(&b.leaves));
    cuts.dedup();
    let kept = cuts.clone();
    cuts.retain(|c| !kept.iter().any(|other| other != c && other.dominates(c)));
}

/// Cut set of a fanin signal: trivial for inputs, empty for constants, computed for gates
fn fanin_cuts(s: Signal, cuts: &[Vec<Cut>]) -> Vec<Cut> {
    if s.is_constant() {
        vec![Cut::empty()]
    } else if s.is_input() {
        vec![Cut::trivial(s)]
    } else {
        cuts[s.var() as usize].clone()
    }
}

/// Merge two cut sets: every feasible union of a cut from each, dominance-pruned
fn merge_cut_sets(a: &[Cut], b: &[Cut], k: usize) -> Vec<Cut> {
    let mut result = Vec::new();
    for ca in a {
        for cb in b {
            if let Some(u) = ca.union(cb, k) {
                result.push(u);
            }
        }
    }
    prune_dominated(&mut result);
    result
}

/// Enumerate k-feasible cuts for every node, with the default priority-cut limit
///
/// Returns one vector of cuts per node, indexed by node (gate) index. Each node's first
/// cut is its trivial self-cut. Primary inputs are not included (their only cut is trivial).
pub fn enumerate_cuts(aig: &Network, max_cut_size: usize) -> Vec<Vec<Cut>> {
    enumerate_cuts_with(aig, max_cut_size, DEFAULT_MAX_CUTS)
}

/// Enumerate k-feasible cuts, keeping at most `max_cuts_per_node` cuts per node
///
/// The trivial self-cut is always kept; among the remaining cuts the smallest are
/// preferred. See [`enumerate_cuts`].
pub fn enumerate_cuts_with(
    aig: &Network,
    max_cut_size: usize,
    max_cuts_per_node: usize,
) -> Vec<Vec<Cut>> {
    assert!(aig.is_topo_sorted());
    assert!(max_cut_size >= 1, "cut size must be at least 1");
    assert!(max_cuts_per_node >= 1, "must keep at least the trivial cut");

    let mut cuts: Vec<Vec<Cut>> = Vec::with_capacity(aig.nb_nodes());
    for i in 0..aig.nb_nodes() {
        let node_sig = Signal::from_var(i as u32);
        let g = aig.gate(i);

        if !g.is_comb() {
            // Flip-flop: combinational boundary, only the trivial cut
            cuts.push(vec![Cut::trivial(node_sig)]);
            continue;
        }

        // Fold-merge the fanin cut sets, starting from a single empty cut
        let mut merged = vec![Cut::empty()];
        for fanin in g.dependencies() {
            let fc = fanin_cuts(*fanin, &cuts);
            merged = merge_cut_sets(&merged, &fc, max_cut_size);
        }
        // Drop empty cuts (only arise from all-constant fanins) and prune
        merged.retain(|c| !c.is_empty());
        prune_dominated(&mut merged);

        // Priority limit: keep the smallest cuts, leaving room for the trivial cut
        let other_limit = max_cuts_per_node - 1;
        if merged.len() > other_limit {
            merged.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.leaves.cmp(&b.leaves)));
            merged.truncate(other_limit);
        }

        let mut node_cuts = Vec::with_capacity(merged.len() + 1);
        node_cuts.push(Cut::trivial(node_sig));
        node_cuts.extend(merged);
        cuts.push(node_cuts);
    }
    cuts
}

/// Verify that a set of leaves is a valid cut of a node
///
/// A cut is valid when every path from the root up to a primary input or flip-flop
/// passes through a leaf. Useful for testing and for validating externally built cuts.
pub fn is_valid_cut(aig: &Network, root: u32, cut: &Cut) -> bool {
    let mut memo = vec![None; aig.nb_nodes()];
    covers(aig, Signal::from_var(root), &cut.leaves, &mut memo)
}

/// Whether all paths from `s` up to the inputs pass through a leaf
fn covers(aig: &Network, s: Signal, leaves: &[Signal], memo: &mut [Option<bool>]) -> bool {
    let s = s.without_inversion();
    if leaves.contains(&s) {
        return true;
    }
    if s.is_constant() {
        return true;
    }
    if s.is_input() {
        // Reached a primary input that is not a leaf: the cut does not cover it
        return false;
    }
    let v = s.var() as usize;
    if let Some(r) = memo[v] {
        return r;
    }
    let g = aig.gate(v);
    // A flip-flop that is not a leaf is a sequential boundary the cut fails to cover
    let r = g.is_comb()
        && g.dependencies()
            .iter()
            .all(|f| covers(aig, *f, leaves, memo));
    memo[v] = Some(r);
    r
}

/// Total number of k-feasible cuts across all nodes (including trivial cuts)
pub fn count_cuts(aig: &Network, max_cut_size: usize) -> usize {
    enumerate_cuts(aig, max_cut_size)
        .iter()
        .map(|c| c.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{count_cuts, enumerate_cuts, enumerate_cuts_with, is_valid_cut, Cut};
    use crate::network::generators::{adder, testcases};
    use crate::{Gate, Network, Signal};

    /// Every cut of every node must be a valid cut, k-feasible, with the trivial cut first
    fn check_all(aig: &Network, k: usize) {
        let cuts = enumerate_cuts(aig, k);
        assert_eq!(cuts.len(), aig.nb_nodes());
        for (i, node_cuts) in cuts.iter().enumerate() {
            assert!(!node_cuts.is_empty(), "node {i} has no cut");
            assert_eq!(
                node_cuts[0],
                Cut::trivial(Signal::from_var(i as u32)),
                "first cut of node {i} should be trivial"
            );
            for c in node_cuts {
                assert!(c.len() <= k, "cut {c} of node {i} exceeds k={k}");
                assert!(
                    is_valid_cut(aig, i as u32, c),
                    "cut {c} of node {i} is invalid"
                );
            }
        }
    }

    #[test]
    fn test_single_and() {
        let mut aig = Network::new();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let o = aig.and(i0, i1);
        aig.add_output(o);

        // k >= 2: trivial cut plus the {i0, i1} cut
        let cuts = enumerate_cuts(&aig, 4);
        assert_eq!(cuts[0].len(), 2);
        assert_eq!(cuts[0][0], Cut::trivial(o));
        let mut leaves = cuts[0][1].leaves().to_vec();
        leaves.sort();
        let mut expected = vec![i0, i1];
        expected.sort();
        assert_eq!(leaves, expected);

        // k == 1: only the trivial cut fits
        let cuts1 = enumerate_cuts(&aig, 1);
        assert_eq!(cuts1[0].len(), 1);
        assert_eq!(cuts1[0][0], Cut::trivial(o));
    }

    #[test]
    fn test_cuts_valid_adder() {
        for k in 2..=6 {
            check_all(&adder::ripple_carry(4), k);
        }
    }

    #[test]
    fn test_cuts_valid_mux_maj_lut() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        let m = aig.add(Gate::mux(a, b, c));
        let j = aig.add(Gate::maj(a, b, c));
        let o = aig.and(m, j);
        aig.add_output(o);
        for k in 2..=4 {
            check_all(&aig, k);
        }
    }

    #[test]
    fn test_two_level() {
        // o = (i0 & i1) & i2, a reconvergent-free 3-input function
        let mut aig = Network::new();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let x = aig.and(i0, i1);
        let o = aig.and(x, i2);
        aig.add_output(o);

        let cuts = enumerate_cuts(&aig, 3);
        let o_cuts = &cuts[o.var() as usize];
        let has = |want: &[Signal]| {
            let mut w = want.to_vec();
            w.sort();
            o_cuts.iter().any(|c| {
                let mut l = c.leaves().to_vec();
                l.sort();
                l == w
            })
        };
        // o has the cuts {x, i2} (across the intermediate gate) and {i0, i1, i2}
        assert!(has(&[x, i2]));
        assert!(has(&[i0, i1, i2]));
    }

    #[test]
    fn test_dff_is_boundary() {
        // A flip-flop output must be a leaf; cuts never cross it
        let aig = testcases::toggle_chain(3, true, true);
        let cuts = enumerate_cuts(&aig, 4);
        for (i, node_cuts) in cuts.iter().enumerate() {
            if !aig.gate(i).is_comb() {
                // A flip-flop has only the trivial cut
                assert_eq!(node_cuts.len(), 1);
                assert_eq!(node_cuts[0], Cut::trivial(Signal::from_var(i as u32)));
            }
        }
        // And all cuts remain valid (covers() rejects crossing a Dff)
        check_all(&aig, 4);
    }

    #[test]
    fn test_priority_limit() {
        // A wide gate would have many cuts; the limit caps them
        let mut aig = Network::new();
        let mut sigs = Vec::new();
        for _ in 0..10 {
            sigs.push(aig.add_input());
        }
        let o = aig.add(Gate::andn(&sigs));
        aig.add_output(o);

        let limit = 5;
        let cuts = enumerate_cuts_with(&aig, 4, limit);
        assert!(cuts[o.var() as usize].len() <= limit);
        // The trivial cut is still present and first
        assert_eq!(cuts[o.var() as usize][0], Cut::trivial(o));
    }

    #[test]
    fn test_count_cuts() {
        let aig = adder::ripple_carry(2);
        // At least one cut (the trivial one) per node
        assert!(count_cuts(&aig, 4) >= aig.nb_nodes());
    }
}
