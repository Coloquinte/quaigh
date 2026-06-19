//! Conversion to a Majority-Inverter Graph (MIG)
//!
//! Lowers every gate to 3-input Majority gates with implicit inversions, the
//! representation used by majority-based logic synthesis. And/Or become a Maj with a
//! constant input (`And(a,b) = Maj(a,b,0)`, `Or(a,b) = Maj(a,b,1)`); Xor, Mux and Lut
//! are decomposed into Maj gates; flip-flops are kept. This is the majority counterpart
//! of the And-based [`to_aig`](super::to_aig) view.
//!
//! Note: the result is intentionally *not* run through canonicalization, because that
//! would rewrite `Maj(a,b,0)` back into an And and destroy the majority view. The
//! simulator and equivalence checker handle Maj-with-constant directly, so the result
//! is still fully usable and verifiable.

use volute::Lut;

use crate::network::{BinaryType, NaryType, TernaryType};
use crate::{Gate, Network, Signal};

/// Translate a signal of the source network into the rebuilt network
fn translate(s: Signal, trans: &[Signal]) -> Signal {
    if s.is_var() {
        trans[s.var() as usize] ^ s.is_inverted()
    } else {
        s
    }
}

/// `a & b` as a majority: `Maj(a, b, 0)`
fn and2(ret: &mut Network, a: Signal, b: Signal) -> Signal {
    ret.add(Gate::maj(a, b, Signal::zero()))
}

/// `a | b` as a majority: `Maj(a, b, 1)`
fn or2(ret: &mut Network, a: Signal, b: Signal) -> Signal {
    ret.add(Gate::maj(a, b, Signal::one()))
}

/// `a ^ b` as majorities: `Maj( a & !b, !a & b, 1 )`
fn xor2(ret: &mut Network, a: Signal, b: Signal) -> Signal {
    let p = and2(ret, a, !b);
    let q = and2(ret, !a, b);
    or2(ret, p, q)
}

/// `s ? a : b` as majorities: `Maj( s & a, !s & b, 1 )`
fn mux2(ret: &mut Network, s: Signal, a: Signal, b: Signal) -> Signal {
    let p = and2(ret, s, a);
    let q = and2(ret, !s, b);
    or2(ret, p, q)
}

/// And of all signals, folded with 2-input majority Ands
fn and_all(ret: &mut Network, sigs: &[Signal]) -> Signal {
    if sigs.is_empty() {
        return Signal::one();
    }
    let mut acc = sigs[0];
    for s in &sigs[1..] {
        acc = and2(ret, acc, *s);
    }
    acc
}

/// Or of all signals, folded with 2-input majority Ors
fn or_all(ret: &mut Network, sigs: &[Signal]) -> Signal {
    if sigs.is_empty() {
        return Signal::zero();
    }
    let mut acc = sigs[0];
    for s in &sigs[1..] {
        acc = or2(ret, acc, *s);
    }
    acc
}

/// Xor of all signals, folded with 2-input majority Xors
fn xor_all(ret: &mut Network, sigs: &[Signal]) -> Signal {
    if sigs.is_empty() {
        return Signal::zero();
    }
    let mut acc = sigs[0];
    for s in &sigs[1..] {
        acc = xor2(ret, acc, *s);
    }
    acc
}

/// Decompose a Lut into majorities through its sum of products (the true minterms)
fn lut_to_mig(ret: &mut Network, lut: &Lut, inputs: &[Signal]) -> Signal {
    let n = lut.num_vars();
    let mut products = Vec::new();
    for mask in 0..lut.num_bits() {
        if lut.value(mask) {
            let lits: Vec<Signal> = (0..n).map(|i| inputs[i] ^ ((mask >> i) & 1 == 0)).collect();
            products.push(and_all(ret, &lits));
        }
    }
    if products.is_empty() {
        return Signal::zero();
    }
    or_all(ret, &products)
}

/// Convert a network to a Majority-Inverter Graph
///
/// All combinatorial logic is expressed with 3-input Maj gates and implicit inverters.
/// Flip-flops are preserved, so sequential networks stay sequential. The result is
/// functionally equivalent to the input.
///
/// ```
/// # use quaigh::Network;
/// use quaigh::optim::to_mig;
/// use quaigh::network::stats::stats;
///
/// let mut net = Network::new();
/// let a = net.add_input();
/// let b = net.add_input();
/// let o = net.and(a, b);
/// net.add_output(o);
///
/// let mig = to_mig(&net);
/// // The And has been expressed as a majority gate
/// assert_eq!(stats(&mig).nb_and, 0);
/// assert!(stats(&mig).nb_maj >= 1);
/// ```
pub fn to_mig(aig: &Network) -> Network {
    // Reduce the gate variety first: Or/Nand/Nor/Xnor become And/Xor and Buf disappears,
    // so only And, Xor, Mux, Maj, Lut and Dff remain to handle below.
    let mut src = aig.clone();
    src.make_canonical();
    assert!(src.is_topo_sorted());

    let mut ret = Network::new();
    ret.add_inputs(src.nb_inputs());
    let mut trans = vec![Signal::placeholder(); src.nb_nodes()];

    // Pre-allocate flip-flops so their output signal exists during the combinatorial pass
    // (a flip-flop input may be driven by a later node).
    for (i, t) in trans.iter_mut().enumerate() {
        if !src.gate(i).is_comb() {
            *t = ret.add(Gate::dff(
                Signal::placeholder(),
                Signal::one(),
                Signal::zero(),
            ));
        }
    }

    // Decompose combinatorial gates in topological order
    for i in 0..src.nb_nodes() {
        let g = src.gate(i);
        if !g.is_comb() {
            continue;
        }
        let deps: Vec<Signal> = g
            .dependencies()
            .iter()
            .map(|s| translate(*s, &trans))
            .collect();
        let s = match g {
            Gate::Binary(_, BinaryType::And) => and2(&mut ret, deps[0], deps[1]),
            Gate::Binary(_, BinaryType::Xor) => xor2(&mut ret, deps[0], deps[1]),
            Gate::Ternary(_, TernaryType::And) => and_all(&mut ret, &deps),
            Gate::Ternary(_, TernaryType::Xor) => xor_all(&mut ret, &deps),
            Gate::Ternary(_, TernaryType::Mux) => mux2(&mut ret, deps[0], deps[1], deps[2]),
            Gate::Ternary(_, TernaryType::Maj) => ret.add(Gate::maj(deps[0], deps[1], deps[2])),
            Gate::Nary(_, NaryType::And) => and_all(&mut ret, &deps),
            Gate::Nary(_, NaryType::Xor) => xor_all(&mut ret, &deps),
            Gate::Lut(lut) => lut_to_mig(&mut ret, &lut.lut, &deps),
            _ => unreachable!("unexpected gate kind after canonicalization: {g}"),
        };
        trans[i] = s;
    }

    // Now that every signal is known, wire up the flip-flop inputs
    for i in 0..src.nb_nodes() {
        if let Gate::Dff([d, en, res]) = src.gate(i) {
            let nd = translate(*d, &trans);
            let nen = translate(*en, &trans);
            let nres = translate(*res, &trans);
            ret.replace(trans[i].var() as usize, Gate::dff(nd, nen, nres));
        }
    }

    for o in 0..src.nb_outputs() {
        ret.add_output(translate(src.output(o), &trans));
    }
    ret.topo_sort();
    // Merge identical Maj nodes without canonicalizing (which would collapse Maj->And)
    ret.deduplicate();
    ret.cleanup();
    ret.check();
    ret
}

#[cfg(test)]
mod tests {
    use volute::Lut3;

    use super::to_mig;
    use crate::equiv::{check_equivalence_bounded, check_equivalence_comb};
    use crate::network::generators::{adder, testcases};
    use crate::network::stats::stats;
    use crate::{Gate, Network};

    /// Assert the network is a pure MIG: only Maj gates (plus flip-flops), no And/Xor/Mux/Lut
    fn assert_pure_mig(aig: &Network) {
        let st = stats(aig);
        assert_eq!(st.nb_and, 0, "And gate remains");
        assert_eq!(st.nb_xor, 0, "Xor gate remains");
        assert_eq!(st.nb_mux, 0, "Mux gate remains");
        assert_eq!(st.nb_lut, 0, "Lut gate remains");
    }

    #[test]
    fn test_to_mig_and() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let o = aig.and(a, b);
        aig.add_output(o);
        let res = to_mig(&aig);
        check_equivalence_comb(&aig, &res, false).unwrap();
        assert_pure_mig(&res);
        assert!(stats(&res).nb_maj >= 1);
    }

    #[test]
    fn test_to_mig_xor3() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        let o = aig.add(Gate::xor3(a, b, c));
        aig.add_output(o);
        let res = to_mig(&aig);
        check_equivalence_comb(&aig, &res, false).unwrap();
        assert_pure_mig(&res);
    }

    #[test]
    fn test_to_mig_mux_maj() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        let m = aig.add(Gate::mux(a, b, c));
        let j = aig.add(Gate::maj(a, b, c));
        aig.add_output(m);
        aig.add_output(j);
        let res = to_mig(&aig);
        check_equivalence_comb(&aig, &res, false).unwrap();
        assert_pure_mig(&res);
    }

    #[test]
    fn test_to_mig_lut() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        let o = aig.add(Gate::lut(&[a, b, c], Lut3::threshold(2).into()));
        aig.add_output(o);
        let res = to_mig(&aig);
        check_equivalence_comb(&aig, &res, false).unwrap();
        assert_pure_mig(&res);
    }

    #[test]
    fn test_to_mig_adder() {
        let aig = adder::ripple_carry(4);
        let res = to_mig(&aig);
        check_equivalence_comb(&aig, &res, false).unwrap();
        assert_pure_mig(&res);
    }

    #[test]
    fn test_to_mig_sequential() {
        let aig = testcases::toggle_chain(4, true, true);
        let res = to_mig(&aig);
        check_equivalence_bounded(&aig, &res, 6, false).unwrap();
        let st = stats(&res);
        assert!(st.nb_dff >= 1, "flip-flops should be preserved");
        assert_eq!(st.nb_and, 0);
        assert_eq!(st.nb_xor, 0);
    }

    #[test]
    fn test_to_mig_idempotent() {
        let aig = adder::ripple_carry(3);
        let m1 = to_mig(&aig);
        let m2 = to_mig(&m1);
        check_equivalence_comb(&m1, &m2, false).unwrap();
        assert_pure_mig(&m2);
    }
}
