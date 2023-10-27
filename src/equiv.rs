//! Bounded equivalence checking on Aigs

use std::{collections::HashMap, hash::Hash};

use cat_solver::Solver;

use crate::{
    aig::Aig,
    gates::Gate,
    signal::{self, Signal},
};

/**
 * Export a combinatorial Aig to a CNF formula
 */
fn to_cnf(aig: &Aig) -> Vec<Vec<Signal>> {
    use Gate::*;
    assert!(aig.is_comb());
    let mut ret = Vec::<Vec<Signal>>::new();
    for i in 0..aig.nb_nodes() {
        let n = aig.node(i);
        match aig.gate(i) {
            And(a, b) => {
                // 3 clauses, 6 literals
                ret.push(vec![a, !n]);
                ret.push(vec![b, !n]);
                ret.push(vec![!a, !b, n]);
            }
            Xor(a, b) => {
                // 4 clauses, 12 literals
                ret.push(vec![a, b, !n]);
                ret.push(vec![!a, !b, !n]);
                ret.push(vec![!a, b, n]);
                ret.push(vec![a, !b, n]);
            }
            And3(a, b, c) => {
                // 4 clauses, 10 literals
                ret.push(vec![a, !n]);
                ret.push(vec![b, !n]);
                ret.push(vec![c, !n]);
                ret.push(vec![!a, !b, !c, n]);
            }
            Xor3(a, b, c) => {
                // 8 clauses, 32 literals
                ret.push(vec![a, b, c, !n]);
                ret.push(vec![a, b, !c, n]);
                ret.push(vec![a, !b, c, n]);
                ret.push(vec![a, !b, !c, !n]);
                ret.push(vec![!a, b, c, n]);
                ret.push(vec![!a, b, !c, !n]);
                ret.push(vec![!a, !b, c, !n]);
                ret.push(vec![!a, !b, !c, n]);
            }
            Mux(s, a, b) => {
                // 4 clauses, 12 literals + 2 redundant clauses
                ret.push(vec![!s, !a, n]);
                ret.push(vec![!s, a, !n]);
                ret.push(vec![s, !b, n]);
                ret.push(vec![s, b, !n]);
                // Redundant but useful
                ret.push(vec![a, b, !n]);
                ret.push(vec![!a, !b, n]);
            }
            Maj(a, b, c) => {
                // 6 clauses, 18 literals
                ret.push(vec![!a, !b, n]);
                ret.push(vec![!b, !c, n]);
                ret.push(vec![!a, !c, n]);
                ret.push(vec![a, b, !n]);
                ret.push(vec![b, c, !n]);
                ret.push(vec![a, c, !n]);
            }
            Dff(_, _, _) => panic!("Combinatorial Aig expected"),
        }
    }
    // Filter out zeros (removed from the clause)
    for c in &mut ret {
        c.retain(|s| *s != Signal::zero());
    }
    // Filter out ones (clause removed)
    ret.retain(|c| c.iter().all(|s| *s != Signal::one()));
    ret
}

/**
 * Copy the gates from one Aig to another and fill the existing translation table
 */
fn extend_aig_helper(a: &mut Aig, b: &Aig, t: &mut HashMap<Signal, Signal>, same_inputs: bool) {
    use Gate::*;
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
        let g = match b.gate(i) {
            And(a, b) => And(t[&a], t[&b]),
            Xor(a, b) => Xor(t[&a], t[&b]),
            And3(a, b, c) => And3(t[&a], t[&b], t[&c]),
            Xor3(a, b, c) => Xor3(t[&a], t[&b], t[&c]),
            Mux(a, b, c) => Mux(t[&a], t[&b], t[&c]),
            Maj(a, b, c) => Maj(t[&a], t[&b], t[&c]),
            Dff(_, _, _) => continue,
        };
        let s = a.add_gate(g);
        t.insert(b.node(i), s);
        t.insert(!b.node(i), !s);
    }
}

/**
 * Copy the gates from one Aig to another and fill the translation table
 */
fn extend_aig(a: &mut Aig, b: &Aig) -> HashMap<Signal, Signal> {
    let mut t = HashMap::<Signal, Signal>::new();
    extend_aig_helper(a, b, &mut t, true);
    t
}

/**
 * Unroll a sequential Aig over a fixed number of steps
 */
fn unroll(aig: &Aig, nb_steps: usize) -> Aig {
    use Gate::*;
    let mut ret = Aig::new();

    let mut t_prev = HashMap::new();
    for step in 0..nb_steps {
        let mut t = HashMap::new();

        // Convert flip-flops for this step
        for i in 0..aig.nb_nodes() {
            if let Dff(d, en, res) = aig.gate(i) {
                let ff = aig.node(i);
                let unroll_ff = if step == 0 {
                    Signal::zero()
                } else {
                    let mx = ret.mux(t_prev[&en], t_prev[&d], t_prev[&ff]);
                    ret.and(mx, !t_prev[&res])
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
    ret
}

/**
 * Create an AIG with a single output, representing the equivalence of two combinatorial Aigs
 */
fn difference(a: &Aig, b: &Aig) -> Aig {
    assert!(a.is_comb() && b.is_comb());
    assert_eq!(a.nb_inputs(), b.nb_inputs());
    assert_eq!(a.nb_outputs(), b.nb_outputs());

    let mut eq = Aig::new();
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
    let equiv = eq.or_n(&outputs);
    eq.add_output(equiv);
    eq
}

/**
 * Find an assignment of the inputs that sets the single output to 1
 */
fn prove(a: &Aig) -> Option<Vec<bool>> {
    assert_eq!(a.nb_outputs(), 1);

    let clauses = to_cnf(a);

    let mut all_lits: Vec<Signal> = clauses.iter().flatten().map(|s| s.without_pol()).collect();
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

/**
 * Perform equivalence checking on two combinatorial AIGs
 */
pub fn check_equivalence_comb(a: &Aig, b: &Aig) -> Result<(), Vec<bool>> {
    assert!(a.is_comb() && b.is_comb());
    let eq = difference(a, b);
    let res = prove(&eq);
    match res {
        None => Ok(()),
        Some(v) => Err(v),
    }
}

/**
 * Perform bounded equivalence checking on two sequential AIGs
 */
pub fn check_equivalence_bounded(a: &Aig, b: &Aig, nb_steps: usize) -> Result<(), Vec<Vec<bool>>> {
    assert_eq!(a.nb_inputs(), b.nb_inputs());
    assert_eq!(a.nb_outputs(), b.nb_outputs());

    let a_u = unroll(a, nb_steps);
    let b_u = unroll(b, nb_steps);

    let res = check_equivalence_comb(&a_u, &b_u);
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
    use crate::{equiv::unroll, Aig, Signal};

    use super::check_equivalence_comb;

    #[test]
    fn test_equiv_and() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let aa = a.and(l1, l2);
        a.add_output(aa);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        let ab = b.and(l1, l2);
        b.add_output(ab);
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_not_equiv_and_zero() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let aa = a.and(l1, l2);
        a.add_output(aa);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        b.add_output(Signal::zero());
        let res = check_equivalence_comb(&a, &b);
        assert_eq!(res, Err(vec![true, true]));
    }

    #[test]
    fn test_not_equiv_one_zero() {
        let mut a = Aig::new();
        a.add_input();
        a.add_input();
        a.add_output(Signal::one());
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        b.add_output(Signal::zero());
        let res = check_equivalence_comb(&a, &b);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_equiv_xor() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let a1 = a.and(l1, !l2);
        let a2 = a.and(!l1, l2);
        let ax = a.or(a1, a2);
        a.add_output(ax);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        let bx = b.xor(l1, l2);
        b.add_output(bx);
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_equiv_mux() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(!l1, l3);
        let ax = a.or(a1, a2);
        a.add_output(ax);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let bx = b.mux(l1, l2, l3);
        b.add_output(bx);
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_equiv_maj() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(l1, l3);
        let a3 = a.and(l2, l3);
        let ax = a.or3(a1, a2, a3);
        a.add_output(ax);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let bx = b.maj(l1, l2, l3);
        b.add_output(bx);
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_equiv_and3() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.and(l1, l2);
        let a2 = a.and(a1, l3);
        a.add_output(a2);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let b2 = b.and3(l1, l2, l3);
        b.add_output(b2);
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_equiv_xor3() {
        let mut a = Aig::new();
        let l1 = a.add_input();
        let l2 = a.add_input();
        let l3 = a.add_input();
        let a1 = a.xor(l1, l2);
        let a2 = a.xor(a1, l3);
        a.add_output(a2);
        let mut b = Aig::new();
        b.add_input();
        b.add_input();
        b.add_input();
        let b2 = b.xor3(l1, l2, l3);
        b.add_output(b2);
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_equiv_inputs() {
        let mut a = Aig::new();
        let mut b = Aig::new();
        for _ in 0..3 {
            let la = a.add_input();
            a.add_output(la);
            let lb = b.add_input();
            b.add_output(lb);
        }
        check_equivalence_comb(&a, &b).unwrap();
    }

    #[test]
    fn test_not_equiv_inputs() {
        let mut a = Aig::new();
        let mut b = Aig::new();
        for _ in 0..3 {
            let la = a.add_input();
            a.add_output(la);
            let lb = b.add_input();
            b.add_output(!lb);
        }
        let res = check_equivalence_comb(&a, &b);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_unused_inputs() {
        let mut a = Aig::new();
        let mut b = Aig::new();
        for _ in 0..3 {
            a.add_input();
            b.add_input();
        }
        let l = Signal::from_input(0);
        a.add_output(l);
        b.add_output(!l);
        let res = check_equivalence_comb(&a, &b);
        assert_ne!(res, Ok(()));
    }

    #[test]
    fn test_simple_unrolling() {
        let mut a = Aig::new();
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
        let mut a = Aig::new();
        let i0 = a.add_input();
        let i1 = a.add_input();
        let d = a.dff(i0, i1, Signal::zero());
        a.add_output(d);

        let nb_steps = 3;
        let un = unroll(&a, nb_steps);
        assert_eq!(un.nb_inputs(), 2 * nb_steps);
        assert_eq!(un.nb_outputs(), nb_steps);
        assert_eq!(un.nb_nodes(), nb_steps - 1);
        assert_eq!(un.nb_mux(), nb_steps - 2);
        assert_eq!(un.nb_and(), 1);
        assert_eq!(un.output(0), Signal::zero());
    }
}
