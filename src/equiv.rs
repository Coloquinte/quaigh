//! Equivalence checking

use std::collections::HashMap;

use cat_solver::Solver;
use volute::Lut;

use crate::network::{BinaryType, NaryType, TernaryType};
use crate::{Gate, Network, Signal};

// TODO: have clean clause builder object to encapsulate this part

/// Add clauses for And-type n-ary function
fn add_and_clauses(
    clauses: &mut Vec<Vec<Signal>>,
    v: &[Signal],
    n: Signal,
    inv_in: bool,
    inv_out: bool,
) {
    for s in v.iter() {
        clauses.push(vec![s ^ inv_in, !n ^ inv_out]);
    }
    let mut c = vec![n ^ inv_out];
    for s in v.iter() {
        c.push(!s ^ inv_in);
    }
    clauses.push(c);
}

/// Add clauses for Xor-type n-ary function
fn add_xor_clauses(
    clauses: &mut Vec<Vec<Signal>>,
    var: &mut u32,
    v: &[Signal],
    n: Signal,
    inv_out: bool,
) {
    if v.is_empty() {
        clauses.push(vec![!n ^ inv_out]);
    } else {
        // Implement as a series of consecutive Xor
        let mut a = v[0];
        for i in 1..v.len() {
            let b = v[i];
            let v = Signal::from_var(*var);
            *var += 1;
            clauses.push(vec![a, b, !v]);
            clauses.push(vec![!a, !b, !v]);
            clauses.push(vec![!a, b, v]);
            clauses.push(vec![a, !b, v]);
            a = v;
        }
        // Final clauses to force n to the last result
        clauses.push(vec![a, !n ^ inv_out]);
        clauses.push(vec![!a, n ^ inv_out]);
    }
}

/// Add clauses for Luts
///
/// This simply adds one clause for each entry of the LUT, so 64 clauses for a 6-input LUT
fn add_lut_clauses(clauses: &mut Vec<Vec<Signal>>, v: &[Signal], n: Signal, lut: &Lut) {
    for mask in 0..lut.num_bits() {
        let val_out = lut.value(mask);
        let mut clause = vec![!n ^ val_out];
        for i in 0..lut.num_vars() {
            let val_i = (mask >> i) & 1 != 0;
            clause.push(v[i] ^ val_i);
        }
        clauses.push(clause);
    }
}

/// Export a combinatorial network to a CNF formula
fn to_cnf(aig: &Network) -> Vec<Vec<Signal>> {
    use Gate::*;
    assert!(aig.is_comb());
    let mut ret = Vec::<Vec<Signal>>::new();
    let mut var = aig.nb_nodes() as u32;
    for i in 0..aig.nb_nodes() {
        let n = aig.node(i);
        match aig.gate(i) {
            Binary([a, b], BinaryType::And) => {
                // 3 clauses, 7 literals
                ret.push(vec![*a, !n]);
                ret.push(vec![*b, !n]);
                ret.push(vec![!a, !b, n]);
            }
            Binary([a, b], BinaryType::Xor) => {
                // 4 clauses, 12 literals
                ret.push(vec![*a, *b, !n]);
                ret.push(vec![!a, !b, !n]);
                ret.push(vec![!a, *b, n]);
                ret.push(vec![*a, !b, n]);
            }
            Ternary([a, b, c], TernaryType::And) => {
                // 4 clauses, 10 literals
                ret.push(vec![*a, !n]);
                ret.push(vec![*b, !n]);
                ret.push(vec![*c, !n]);
                ret.push(vec![!a, !b, !c, n]);
            }
            Ternary([a, b, c], TernaryType::Xor) => {
                // 8 clauses, 24 literals, one new variable
                let v = Signal::from_var(var);
                var += 1;
                // First Xor to new variable
                ret.push(vec![*a, *b, !v]);
                ret.push(vec![!a, !b, !v]);
                ret.push(vec![!a, *b, v]);
                ret.push(vec![*a, !b, v]);
                // Second Xor to output
                ret.push(vec![v, *c, !n]);
                ret.push(vec![!v, !c, !n]);
                ret.push(vec![!v, *c, n]);
                ret.push(vec![v, !c, n]);
            }
            Ternary([s, a, b], TernaryType::Mux) => {
                // 4 clauses, 12 literals + 2 redundant clauses
                ret.push(vec![!s, !a, n]);
                ret.push(vec![!s, *a, !n]);
                ret.push(vec![*s, !b, n]);
                ret.push(vec![*s, *b, !n]);
                // Redundant but useful
                ret.push(vec![*a, *b, !n]);
                ret.push(vec![!a, !b, n]);
            }
            Ternary([a, b, c], TernaryType::Maj) => {
                // 6 clauses, 18 literals
                ret.push(vec![!a, !b, n]);
                ret.push(vec![!b, !c, n]);
                ret.push(vec![!a, !c, n]);
                ret.push(vec![*a, *b, !n]);
                ret.push(vec![*b, *c, !n]);
                ret.push(vec![*a, *c, !n]);
            }
            Dff(_) => panic!("Combinatorial network expected"),
            Nary(v, tp) => match tp {
                NaryType::And => add_and_clauses(&mut ret, v, n, false, false),
                NaryType::Or => add_and_clauses(&mut ret, v, n, true, true),
                NaryType::Nand => add_and_clauses(&mut ret, v, n, false, true),
                NaryType::Nor => add_and_clauses(&mut ret, v, n, true, false),
                NaryType::Xor => add_xor_clauses(&mut ret, &mut var, v, n, false),
                NaryType::Xnor => add_xor_clauses(&mut ret, &mut var, v, n, true),
            },
            Buf(s) => {
                ret.push(vec![*s, !n]);
                ret.push(vec![!s, n]);
            }
            Lut(lut) => {
                add_lut_clauses(&mut ret, &lut.inputs, n, &lut.lut);
            }
        }
    }
    // Filter out zeros (removed from the clause)
    for c in &mut ret {
        c.retain(|s| *s != Signal::zero());
        c.sort();
        c.dedup();
    }
    // Filter out ones (clause removed)
    ret.retain(|c| c.iter().all(|s| *s != Signal::one()));
    ret
}

/// Copy the gates from one network to another and fill the existing translation table
fn extend_aig_helper(
    a: &mut Network,
    b: &Network,
    t: &mut HashMap<Signal, Signal>,
    same_inputs: bool,
) {
    assert!(b.is_topo_sorted());
    assert!(!same_inputs || a.nb_inputs() == b.nb_inputs());

    t.insert(Signal::zero(), Signal::zero());
    t.insert(Signal::one(), Signal::one());
    for i in 0..b.nb_inputs() {
        let sa = if same_inputs {
            a.input(i)
        } else {
            a.add_input()
        };
        let sb = b.input(i);
        t.insert(sb, sa);
        t.insert(!sb, !sa);
    }
    for i in 0..b.nb_nodes() {
        if !b.gate(i).is_comb() {
            continue;
        }
        let g = b.gate(i).remap(|s| t[s]);
        let s = a.add(g);
        t.insert(b.node(i), s);
        t.insert(!b.node(i), !s);
    }
}

/// Copy the gates from one network to another and fill the translation table
fn extend_aig(a: &mut Network, b: &Network) -> HashMap<Signal, Signal> {
    let mut t = HashMap::<Signal, Signal>::new();
    extend_aig_helper(a, b, &mut t, true);
    t
}

/// Unroll a sequential network over a fixed number of steps, making a larger combinatorial networks
pub fn unroll(aig: &Network, nb_steps: usize) -> Network {
    use Gate::*;
    let mut ret = Network::new();

    let mut t_prev = HashMap::new();
    for step in 0..nb_steps {
        let mut t = HashMap::new();

        // Convert flip-flops for this step
        for i in 0..aig.nb_nodes() {
            if let Dff([d, en, res]) = aig.gate(i) {
                let ff = aig.node(i);
                let unroll_ff = if step == 0 {
                    Signal::zero()
                } else {
                    let mx = ret.add_canonical(Gate::mux(t_prev[en], t_prev[d], t_prev[&ff]));
                    ret.and(mx, !t_prev[res])
                };
                t.insert(ff, unroll_ff);
                t.insert(!ff, !unroll_ff);
            }
        }

        // Convert inputs and nodes
        extend_aig_helper(&mut ret, aig, &mut t, false);

        for o in 0..aig.nb_outputs() {
            ret.add_output(t[&aig.output(o)]);
        }
        std::mem::swap(&mut t, &mut t_prev);
    }
    assert_eq!(ret.nb_inputs(), aig.nb_inputs() * nb_steps);
    assert_eq!(ret.nb_outputs(), aig.nb_outputs() * nb_steps);
    ret
}

/// Create a network with a single output, representing whether two combinatorial networks give different outputs
pub fn difference(a: &Network, b: &Network) -> Network {
    assert!(a.is_comb() && b.is_comb());
    assert_eq!(a.nb_inputs(), b.nb_inputs());
    assert_eq!(a.nb_outputs(), b.nb_outputs());

    let mut eq = Network::new();
    eq.add_inputs(a.nb_inputs());
    let ta = extend_aig(&mut eq, a);
    let tb = extend_aig(&mut eq, b);

    let mut outputs = Vec::new();
    for i in 0..a.nb_outputs() {
        let sa = ta[&a.output(i)];
        let sb = tb[&b.output(i)];
        let o = eq.xor(sa, sb);
        outputs.push(o);
    }
    let diff = eq.add_canonical(Gate::Nary(outputs.into(), NaryType::Or));
    eq.add_output(diff);
    eq
}

/// Find an assignment of the inputs that sets the single output to 1
///
/// Returns the assignment, or None if no such assignment exists.
pub fn prove(a: &Network) -> Option<Vec<bool>> {
    assert_eq!(a.nb_outputs(), 1);

    let clauses = to_cnf(a);

    let mut all_lits: Vec<Signal> = clauses
        .iter()
        .flatten()
        .map(|s| s.without_inversion())
        .collect();
    for i in 0..a.nb_inputs() {
        all_lits.push(Signal::from_input(i as u32));
    }
    all_lits.sort();
    all_lits.dedup();

    let mut t = HashMap::new();
    let mut i: i32 = 1;
    for s in all_lits {
        t.insert(s, i);
        t.insert(!s, -i);
        i += 1;
    }

    let mut solver = Solver::new();
    for c in clauses {
        let clause: Vec<i32> = c.iter().map(|s| t[s]).collect();
        solver.add_clause(clause);
    }
    let out = a.output(0);
    if out == Signal::one() {
        return Some(vec![false; a.nb_inputs()]);
    } else if out == Signal::zero() {
        return None;
    }
    solver.add_clause([t[&out]]);

    match solver.solve() {
        None => panic!("Couldn't solve SAT problem"),
        Some(false) => None,
        Some(true) => {
            let mut v = Vec::new();
            for inp in 0..a.nb_inputs() {
                let b = solver
                    .value(t[&Signal::from_input(inp as u32)])
                    .unwrap_or(false);
                v.push(b);
            }
            Some(v)
        }
    }
}

/// Perform equivalence checking on two combinatorial networks
pub fn check_equivalence_comb(a: &Network, b: &Network, optimize: bool) -> Result<(), Vec<bool>> {
    assert!(a.is_comb() && b.is_comb());
    let mut diff = difference(a, b);
    if optimize {
        diff.make_canonical();
        diff.cleanup();
    }
    let res = prove(&diff);
    match res {
        None => Ok(()),
        Some(v) => Err(v),
    }
}

/// Perform bounded equivalence checking on two sequential networks
pub fn check_equivalence_bounded(
    a: &Network,
    b: &Network,
    nb_steps: usize,
    optimize: bool,
) -> Result<(), Vec<Vec<bool>>> {
    assert_eq!(a.nb_inputs(), b.nb_inputs());
    assert_eq!(a.nb_outputs(), b.nb_outputs());

    let a_u = unroll(a, nb_steps);
    let b_u = unroll(b, nb_steps);

    let res = check_equivalence_comb(&a_u, &b_u, optimize);
    match res {
        Ok(()) => Ok(()),
        Err(v) => {
            assert_eq!(v.len(), a.nb_inputs() * nb_steps);
            let mut assignment = Vec::<Vec<bool>>::new();
            for step in 0..nb_steps {
                let b = step * a.nb_inputs();
                let e = (step + 1) * a.nb_inputs();
                assignment.push(v[b..e].to_vec());
            }
            Err(assignment)
        }
    }
}

#[cfg(test)]
mod tests {
    use volute::Lut;

    use crate::equiv::unroll;
    use crate::network::stats::stats;
    use crate::network::NaryType;
    use crate::{Gate, Network, Signal};

    use super::{check_equivalence_comb, prove};

    #[test]
    fn test_equiv_and() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let aa = a.and(l1, l2);
        a.add_output(aa);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        let ab = b.and(l1, l2);
        b.add_output(ab);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_not_equiv_and_zero() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let aa = a.and(l1, l2);
        a.add_output(aa);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_output(Signal::zero());
        let res = check_equivalence_comb(&a, &b, false);
        assert_eq!(res, Err(vec![true, true]));
    }

    #[test]
    fn test_not_equiv_and_or() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let aa = a.and(l1, l2);
        a.add_output(aa);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        let ab = !b.and(!l1, !l2);
        b.add_output(ab);
        let res = check_equivalence_comb(&a, &b, false);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_not_equiv_one_zero() {
        let mut a = Network::new();
        a.add_input();
        a.add_input();
        a.add_output(Signal::one());
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_output(Signal::zero());
        let res = check_equivalence_comb(&a, &b, false);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_equiv_xor() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let a1 = a.and(l1, !l2);
        let a2 = a.and(!l1, l2);
        let ax = !a.and(!a1, !a2);
        a.add_output(ax);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        let bx = b.xor(l1, l2);
        b.add_output(bx);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_mux() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(!l1, l3);
        let ax = !a.and(!a1, !a2);
        a.add_output(ax);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let bx = b.add_canonical(Gate::mux(l1, l2, l3));
        b.add_output(bx);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_maj() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(l1, l3);
        let a3 = a.and(l2, l3);
        let ax = !a.add(Gate::and3(!a1, !a2, !a3));
        a.add_output(ax);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let bx = b.add(Gate::maj(l1, l2, l3));
        b.add_output(bx);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_and3() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(a1, l3);
        a.add_output(a2);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let b2 = b.add(Gate::and3(l1, l2, l3));
        b.add_output(b2);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_xor3() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.xor(l1, l2);
        let a2 = a.xor(a1, l3);
        a.add_output(a2);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let b2 = b.add(Gate::xor3(l1, l2, l3));
        b.add_output(b2);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_andn() {
        for nb in 0..8 {
            let mut a = Network::new();
            let mut ao = Signal::one();
            for _ in 0..nb {
                let inp = a.add_input();
                ao = a.and(ao, inp);
            }
            a.add_output(ao);

            let mut b = Network::new();
            let mut v = Vec::new();
            for _ in 0..nb {
                v.push(b.add_input());
            }
            let bo = b.add(Gate::Nary(v.into(), NaryType::And));
            b.add_output(bo);
            check_equivalence_comb(&a, &b, false).unwrap();
            check_equivalence_comb(&a, &b, true).unwrap();
        }
    }

    #[test]
    fn test_equiv_xorn() {
        for nb in 0..8 {
            let mut a = Network::new();
            let mut ao = Signal::zero();
            for _ in 0..nb {
                let inp = a.add_input();
                ao = a.xor(ao, inp);
            }
            a.add_output(ao);

            let mut b = Network::new();
            let mut v = Vec::new();
            for _ in 0..nb {
                v.push(b.add_input());
            }
            let bo = b.add(Gate::Nary(v.into(), NaryType::Xor));
            b.add_output(bo);
            check_equivalence_comb(&a, &b, false).unwrap();
            check_equivalence_comb(&a, &b, true).unwrap();
        }
    }

    #[test]
    fn test_equiv_inputs() {
        let mut a = Network::new();
        let mut b = Network::new();
        for _ in 0..3 {
            let la = a.add_input();
            a.add_output(la);
            let lb = b.add_input();
            b.add_output(lb);
        }
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_not_equiv_inputs() {
        let mut a = Network::new();
        let mut b = Network::new();
        for _ in 0..3 {
            let la = a.add_input();
            a.add_output(la);
            let lb = b.add_input();
            b.add_output(!lb);
        }
        let res = check_equivalence_comb(&a, &b, false);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_unused_inputs() {
        let mut a = Network::new();
        let mut b = Network::new();
        for _ in 0..3 {
            a.add_input();
            b.add_input();
        }
        let l = Signal::from_input(0);
        a.add_output(l);
        b.add_output(!l);
        let res = check_equivalence_comb(&a, &b, false);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_simple_unrolling() {
        let mut a = Network::new();
        let i0 = a.add_input();
        let d = a.dff(i0, Signal::one(), Signal::zero());
        a.add_output(d);

        let nb_steps = 3;
        let un = unroll(&a, nb_steps);
        assert_eq!(un.nb_inputs(), nb_steps);
        assert_eq!(un.nb_outputs(), nb_steps);
        assert_eq!(un.nb_nodes(), 0);
        assert_eq!(un.output(0), Signal::zero());
        for i in 1..nb_steps {
            assert_eq!(un.output(i), un.input(i - 1));
        }
    }

    #[test]
    fn test_enable_unrolling() {
        let mut a = Network::new();
        let i0 = a.add_input();
        let i1 = a.add_input();
        let d = a.dff(i0, i1, Signal::zero());
        a.add_output(d);

        let nb_steps = 3;
        let un = unroll(&a, nb_steps);
        assert_eq!(un.nb_inputs(), 2 * nb_steps);
        assert_eq!(un.nb_outputs(), nb_steps);
        assert_eq!(un.nb_nodes(), nb_steps - 1);
        let st = stats(&un);
        assert_eq!(st.nb_mux, nb_steps - 2);
        assert_eq!(st.nb_and, 1);
        assert_eq!(un.output(0), Signal::zero());
    }

    #[test]
    fn test_prove_and() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        // Add an unused input
        a.add_input();
        let aa = a.and(l1, l2);
        a.add_output(aa);
        let p = prove(&a).unwrap();
        assert_eq!(p.len(), 3);
        assert!(p[0]);
        assert!(p[1]);
    }

    #[test]
    fn test_equiv_lut_xor3() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.xor(l1, l2);
        let a2 = a.xor(a1, l3);
        a.add_output(a2);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let lut = Lut::nth_var(3, 0) ^ Lut::nth_var(3, 1) ^ Lut::nth_var(3, 2);
        let b2 = b.add(Gate::lut(&[l1, l2, l3], lut));
        b.add_output(b2);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_lut_and3() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(a1, l3);
        a.add_output(a2);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let lut = Lut::nth_var(3, 0) & Lut::nth_var(3, 1) & Lut::nth_var(3, 2);
        let b2 = b.add(Gate::lut(&[l1, l2, l3], lut));
        b.add_output(b2);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_lut_andinv3() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a2 = a.add(Gate::and3(!l1, !l2, l3));
        a.add_output(a2);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let lut = !Lut::nth_var(3, 0) & !Lut::nth_var(3, 1) & Lut::nth_var(3, 2);
        let b2 = b.add(Gate::lut(&[l1, l2, l3], lut));
        b.add_output(b2);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }

    #[test]
    fn test_equiv_lut_inv_inputs() {
        let mut a = Network::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a2 = a.add(Gate::and3(!l1, !l2, !l3));
        a.add_output(a2);
        let mut b = Network::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let lut = Lut::nth_var(3, 0) & Lut::nth_var(3, 1) & Lut::nth_var(3, 2);
        let b2 = b.add(Gate::lut(&[!l1, !l2, !l3], lut));
        b.add_output(b2);
        check_equivalence_comb(&a, &b, false).unwrap();
        check_equivalence_comb(&a, &b, true).unwrap();
    }
}
