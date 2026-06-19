//! Conversion to an And-Inverter Graph (AIG)
//!
//! Lowers every gate to 2-input And gates with implicit inversions, the classic
//! AIG representation used by tools such as [ABC](https://github.com/berkeley-abc/abc).
//! Xor, Mux, Maj and Lut gates are decomposed into Ands; flip-flops are kept, giving
//! an AIG with sequential elements. This provides a uniform And-based view, useful as
//! a normal form and as a basis for And-graph algorithms and faster simulation.

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

/// `a ^ b` expressed with And gates: `!( !(a & !b) & !(!a & b) )`
fn xor2(ret: &mut Network, a: Signal, b: Signal) -> Signal {
    let p = ret.and(a, !b);
    let q = ret.and(!a, b);
    !ret.and(!p, !q)
}

/// `s ? a : b` expressed with And gates: `!( !(s & a) & !(!s & b) )`
fn mux2(ret: &mut Network, s: Signal, a: Signal, b: Signal) -> Signal {
    let p = ret.and(s, a);
    let q = ret.and(!s, b);
    !ret.and(!p, !q)
}

/// `Maj(a, b, c)` expressed with And gates: `!( !(a&b) & !(b&c) & !(a&c) )`
fn maj3(ret: &mut Network, a: Signal, b: Signal, c: Signal) -> Signal {
    let ab = ret.and(a, b);
    let bc = ret.and(b, c);
    let ac = ret.and(a, c);
    let t = ret.and(!ab, !bc);
    !ret.and(t, !ac)
}

/// And of all signals, as a left-leaning tree of 2-input Ands
fn and_all(ret: &mut Network, sigs: &[Signal]) -> Signal {
    let mut acc = Signal::one();
    for s in sigs {
        acc = ret.and(acc, *s);
    }
    acc
}

/// Xor of all signals, folded with 2-input Xors
fn xor_all(ret: &mut Network, sigs: &[Signal]) -> Signal {
    let mut acc = Signal::zero();
    for s in sigs {
        acc = xor2(ret, acc, *s);
    }
    acc
}

/// Decompose a Lut into Ands through its sum of products (the true minterms)
fn lut_to_and(ret: &mut Network, lut: &Lut, inputs: &[Signal]) -> Signal {
    let n = lut.num_vars();
    let mut products = Vec::new();
    for mask in 0..lut.num_bits() {
        if lut.value(mask) {
            // Product term: a literal per input, complemented where the minterm bit is 0
            let lits: Vec<Signal> = (0..n).map(|i| inputs[i] ^ ((mask >> i) & 1 == 0)).collect();
            products.push(and_all(ret, &lits));
        }
    }
    if products.is_empty() {
        return Signal::zero();
    }
    // Or of the products: `!( And of !products )`
    let inv: Vec<Signal> = products.iter().map(|s| !*s).collect();
    !and_all(ret, &inv)
}

/// Convert a network to a 2-input And-Inverter Graph
///
/// All combinatorial logic is expressed with 2-input And gates and implicit inverters.
/// Flip-flops are preserved, so sequential networks stay sequential. The result is
/// functionally equivalent to the input.
///
/// ```
/// # use quaigh::{Gate, Network};
/// use quaigh::optim::to_aig;
/// use quaigh::network::stats::stats;
///
/// let mut net = Network::new();
/// let a = net.add_input();
/// let b = net.add_input();
/// let c = net.add_input();
/// let o = net.add(Gate::xor3(a, b, c));
/// net.add_output(o);
///
/// let aig = to_aig(&net);
/// // The Xor has been lowered to And gates
/// assert_eq!(stats(&aig).nb_xor, 0);
/// ```
pub fn to_aig(aig: &Network) -> Network {
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
            Gate::Binary(_, BinaryType::And) => ret.and(deps[0], deps[1]),
            Gate::Binary(_, BinaryType::Xor) => xor2(&mut ret, deps[0], deps[1]),
            Gate::Ternary(_, TernaryType::And) => and_all(&mut ret, &deps),
            Gate::Ternary(_, TernaryType::Xor) => xor_all(&mut ret, &deps),
            Gate::Ternary(_, TernaryType::Mux) => mux2(&mut ret, deps[0], deps[1], deps[2]),
            Gate::Ternary(_, TernaryType::Maj) => maj3(&mut ret, deps[0], deps[1], deps[2]),
            Gate::Nary(_, NaryType::And) => and_all(&mut ret, &deps),
            Gate::Nary(_, NaryType::Xor) => xor_all(&mut ret, &deps),
            Gate::Lut(lut) => lut_to_and(&mut ret, &lut.lut, &deps),
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
    ret.make_canonical();
    ret.cleanup();
    ret
}

#[cfg(test)]
mod tests {
    use volute::Lut3;

    use super::to_aig;
    use crate::equiv::{check_equivalence_bounded, check_equivalence_comb};
    use crate::network::generators::{adder, testcases};
    use crate::network::stats::stats;
    use crate::{Gate, Network};

    /// Assert the network is a pure 2-input AIG: only 2-input Ands (plus flip-flops)
    fn assert_pure_aig(aig: &Network) {
        let st = stats(aig);
        assert_eq!(st.nb_xor, 0, "Xor gate remains");
        assert_eq!(st.nb_mux, 0, "Mux gate remains");
        assert_eq!(st.nb_maj, 0, "Maj gate remains");
        assert_eq!(st.nb_lut, 0, "Lut gate remains");
        for (arity, nb) in st.and_arity.iter().enumerate() {
            if arity != 2 {
                assert_eq!(*nb, 0, "And of arity {arity} remains");
            }
        }
    }

    #[test]
    fn test_to_aig_xor3() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        let o = aig.add(Gate::xor3(a, b, c));
        aig.add_output(o);
        let res = to_aig(&aig);
        check_equivalence_comb(&aig, &res, true).unwrap();
        assert_pure_aig(&res);
    }

    #[test]
    fn test_to_aig_mux_maj() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        let m = aig.add(Gate::mux(a, b, c));
        let j = aig.add(Gate::maj(a, b, c));
        aig.add_output(m);
        aig.add_output(j);
        let res = to_aig(&aig);
        check_equivalence_comb(&aig, &res, true).unwrap();
        assert_pure_aig(&res);
    }

    #[test]
    fn test_to_aig_lut() {
        let mut aig = Network::new();
        let a = aig.add_input();
        let b = aig.add_input();
        let c = aig.add_input();
        // 3-input majority as a Lut, exercising the sum-of-products decomposition
        let o = aig.add(Gate::lut(&[a, b, c], Lut3::threshold(2).into()));
        aig.add_output(o);
        let res = to_aig(&aig);
        check_equivalence_comb(&aig, &res, true).unwrap();
        assert_pure_aig(&res);
    }

    #[test]
    fn test_to_aig_adder() {
        let aig = adder::ripple_carry(4);
        let res = to_aig(&aig);
        check_equivalence_comb(&aig, &res, true).unwrap();
        assert_pure_aig(&res);
    }

    #[test]
    fn test_to_aig_sequential() {
        let aig = testcases::toggle_chain(4, true, true);
        let res = to_aig(&aig);
        check_equivalence_bounded(&aig, &res, 6, true).unwrap();
        let st = stats(&res);
        assert!(st.nb_dff >= 1, "flip-flops should be preserved");
        assert_eq!(st.nb_xor, 0);
    }

    #[test]
    fn test_to_aig_idempotent() {
        let aig = adder::ripple_carry(3);
        let a1 = to_aig(&aig);
        let a2 = to_aig(&a1);
        check_equivalence_comb(&a1, &a2, true).unwrap();
        assert_pure_aig(&a2);
    }
}
