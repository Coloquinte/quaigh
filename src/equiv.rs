//! Bounded equivalence checking on Aigs

use std::collections::HashMap;

use cat_solver::Solver;

use crate::{aig::Aig, gates::Gate, signal::Signal};

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
                ret.push(vec![!a, !b, n]);
                ret.push(vec![a, b, !n]);
                ret.push(vec![b, c, !n]);
                ret.push(vec![a, b, !n]);
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
 * Copy the gates from one Aig to another and fill the translation table
 */
fn extend_aig(a: &mut Aig, b: &Aig) -> HashMap<Signal, Signal> {
    use Gate::*;
    assert_eq!(a.nb_inputs(), b.nb_inputs());
    assert!(b.is_comb());
    assert!(b.is_topo_sorted());

    let mut t = HashMap::<Signal, Signal>::new();
    for i in 0..b.nb_inputs() {
        let s = Signal::from_input(i as u32);
        t.insert(s, s);
        t.insert(!s, !s);
    }
    for i in 0..b.nb_nodes() {
        let g = match b.gate(i) {
            And(a, b) => And(t[&a], t[&b]),
            Xor(a, b) => Xor(t[&a], t[&b]),
            And3(a, b, c) => And3(t[&a], t[&b], t[&c]),
            Xor3(a, b, c) => Xor3(t[&a], t[&b], t[&c]),
            Mux(a, b, c) => Mux(t[&a], t[&b], t[&c]),
            Maj(a, b, c) => Maj(t[&a], t[&b], t[&c]),
            Dff(_, _, _) => panic!("Unexpected flip-flop"),
        };
        let s = a.add_gate(g);
        t.insert(b.node(i), s);
        t.insert(!b.node(i), !s);
    }
    t
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
        println!("{:?}", clause);
        solver.add_clause(clause);
    }
    for o in 0..a.nb_outputs() {
        println!("{:?}", vec![t[&a.output(o)]]);
        solver.add_clause([t[&a.output(o)]]);
    }

    match solver.solve() {
        None => panic!("Couldn't solve SAT problem"),
        Some(false) => None,
        Some(true) => {
            // TODO: check what happens if some inputs are unused
            let mut v = Vec::new();
            for lit in 1..i {
                print!("{}, ", solver.value(lit).unwrap_or(false));
            }
            println!();
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
 * Unroll a sequential Aig over a fixed number of steps
 */
fn unroll(aig: &Aig, nb_steps: usize) -> Aig {
    // TODO
    aig.clone()
}

/**
 * Perform equivalence checking on two combinatorial AIGs
 */
pub fn check_equivalence_comb(a: &Aig, b: &Aig) -> Result<(), Vec<bool>> {
    assert!(a.is_comb() && b.is_comb());
    let eq = difference(a, b);
    println!("A: {a}");
    println!("B: {b}");
    println!("Eq: {eq}");
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
    use crate::Aig;

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
}
