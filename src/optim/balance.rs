//! Depth-oriented balancing of And and Xor trees
//!
//! Restructures associative And and Xor gates into minimum-depth trees of 2-input
//! gates, reducing the combinational depth of the network without changing its
//! function. This complements [`share_logic`](super::share_logic), which optimizes
//! for area (sharing) rather than depth.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::{Gate, Network, Signal};

use super::share_logic::flatten_nary;

/// Logic level of a signal given the levels of all nodes
fn signal_level(s: Signal, level: &[u32]) -> u32 {
    if s.is_var() {
        level[s.var() as usize]
    } else {
        0
    }
}

/// Build a minimum-depth tree of 2-input gates computing the And or Xor of the leaves
///
/// Repeatedly combines the two lowest-level signals, which minimizes the depth of the
/// resulting tree. New gates are appended to `ret`, and their level is recorded in
/// `level` so the invariant `level.len() == ret.nb_nodes()` is maintained.
fn build_tree(ret: &mut Network, level: &mut Vec<u32>, leaves: &[Signal], is_and: bool) -> Signal {
    debug_assert!(!leaves.is_empty());
    // Min-heap keyed by (level, tie-breaker, signal): always combine the two shallowest
    // signals first. The tie-breaker keeps the construction deterministic.
    let mut heap = BinaryHeap::new();
    let mut tie = 0u32;
    for &s in leaves {
        heap.push(Reverse((signal_level(s, level), tie, s)));
        tie += 1;
    }
    while heap.len() >= 2 {
        let Reverse((la, _, a)) = heap.pop().unwrap();
        let Reverse((lb, _, b)) = heap.pop().unwrap();
        let g = if is_and {
            Gate::and(a, b)
        } else {
            Gate::xor(a, b)
        };
        let s = ret.add(g);
        debug_assert_eq!(s.var() as usize, level.len());
        let new_level = la.max(lb) + 1;
        level.push(new_level);
        heap.push(Reverse((new_level, tie, s)));
        tie += 1;
    }
    let Reverse((_, _, root)) = heap.pop().unwrap();
    root
}

/// Balance And and Xor trees to reduce the combinational depth of the network
///
/// Functionality is preserved. Other gates (Mux, Maj, Dff, Lut) are left untouched.
/// The transformation is deterministic.
///
/// ```
/// # use quaigh::{Gate, Network};
/// use quaigh::optim::balance;
/// use quaigh::network::stats::depth;
///
/// // A right-leaning And chain of 8 inputs is 7 levels deep
/// let mut aig = Network::new();
/// let mut sigs = Vec::new();
/// for _ in 0..8 {
///     sigs.push(aig.add_input());
/// }
/// let mut acc = sigs[0];
/// for s in &sigs[1..] {
///     acc = aig.and(acc, *s);
/// }
/// aig.add_output(acc);
/// assert_eq!(depth(&aig), 7);
///
/// // Balancing turns it into a tree, reducing the depth to 3
/// let balanced = balance(&aig);
/// assert!(depth(&balanced) <= 3);
/// ```
pub fn balance(aig: &Network) -> Network {
    balance_with(aig, 64)
}

/// Balance with an explicit flattening limit; see [`flatten_nary`]
pub fn balance_with(aig: &Network, max_size: usize) -> Network {
    // Flatten associative chains into N-ary gates so a whole chain is rebuilt at once
    let flat = flatten_nary(aig, max_size);
    assert!(flat.is_topo_sorted());

    let mut ret = flat.clone();
    // Combinational level of each node, grown as balanced-tree gates are appended.
    // Processing in topological order means every gate input level is known when used.
    let mut level = vec![0u32; flat.nb_nodes()];

    for i in 0..flat.nb_nodes() {
        let g = flat.gate(i).clone();
        if g.is_and() || g.is_xor() {
            let leaves: Vec<Signal> = g.dependencies().to_vec();
            let root = build_tree(&mut ret, &mut level, &leaves, g.is_and());
            ret.replace(i, Gate::Buf(root));
            level[i] = signal_level(root, &level);
        } else if g.is_comb() {
            let mut m = 0;
            for v in g.vars() {
                m = m.max(level[v as usize]);
            }
            level[i] = if g.is_buf_like() { m } else { m + 1 };
        } else {
            // Flip-flop output: sequential source at level 0
            level[i] = 0;
        }
    }

    // The Buf placeholders reference appended gates, so re-sort, then canonicalize the
    // freshly added 2-input gates and drop the now-unused original gates.
    ret.topo_sort();
    ret.make_canonical();
    ret.cleanup();
    ret
}

#[cfg(test)]
mod tests {
    use super::{balance, balance_with};
    use crate::equiv::check_equivalence_comb;
    use crate::network::generators::adder;
    use crate::network::stats::depth;
    use crate::Network;

    fn deep_and_chain(n: usize) -> Network {
        let mut aig = Network::new();
        let mut sigs = Vec::new();
        for _ in 0..n {
            sigs.push(aig.add_input());
        }
        let mut acc = sigs[0];
        for s in &sigs[1..] {
            acc = aig.and(acc, *s);
        }
        aig.add_output(acc);
        aig
    }

    fn deep_xor_chain(n: usize) -> Network {
        let mut aig = Network::new();
        let mut sigs = Vec::new();
        for _ in 0..n {
            sigs.push(aig.add_input());
        }
        let mut acc = sigs[0];
        for s in &sigs[1..] {
            acc = aig.xor(acc, *s);
        }
        aig.add_output(acc);
        aig
    }

    #[test]
    fn test_balance_and_chain_reduces_depth() {
        let aig = deep_and_chain(8);
        assert_eq!(depth(&aig), 7);
        let balanced = balance(&aig);
        check_equivalence_comb(&aig, &balanced, true).unwrap();
        assert!(depth(&balanced) <= 3, "depth was {}", depth(&balanced));
    }

    #[test]
    fn test_balance_xor_chain_reduces_depth() {
        let aig = deep_xor_chain(8);
        assert_eq!(depth(&aig), 7);
        let balanced = balance(&aig);
        check_equivalence_comb(&aig, &balanced, true).unwrap();
        assert!(depth(&balanced) <= 3, "depth was {}", depth(&balanced));
    }

    #[test]
    fn test_balance_preserves_adder() {
        let aig = adder::ripple_carry(4);
        let balanced = balance(&aig);
        check_equivalence_comb(&aig, &balanced, true).unwrap();
    }

    #[test]
    fn test_balance_is_deterministic() {
        let aig = deep_and_chain(20);
        let b1 = balance(&aig);
        let b2 = balance(&aig);
        assert_eq!(format!("{b1}"), format!("{b2}"));
    }

    #[test]
    fn test_balance_is_stable() {
        // Balancing an already-balanced network keeps it equivalent
        let aig = deep_and_chain(16);
        let b1 = balance(&aig);
        let b2 = balance_with(&b1, 64);
        check_equivalence_comb(&b1, &b2, true).unwrap();
    }
}
