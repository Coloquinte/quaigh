//! Infer Xor and Mux gates from And gates

use crate::network::matcher::Matcher;
use crate::{Gate, Network, Signal};

fn mux_pattern() -> Network {
    let mut pattern = Network::new();
    let s = pattern.add_input();
    let a = pattern.add_input();
    let b = pattern.add_input();
    let x0 = pattern.add(Gate::and(s, !a));
    let x1 = pattern.add(Gate::and(!s, !b));
    let o = pattern.add(Gate::and(!x0, !x1));
    pattern.add_output(o);
    pattern
}

/// Rebuild Xor and Mux gates from And gates
pub fn infer_xor_mux(aig: &mut Network) {
    let mut ret = aig.clone();

    let pattern = mux_pattern();
    let mut matcher = Matcher::from_pattern(&pattern);
    for i in 0..ret.nb_nodes() {
        if let Some(v) = matcher.matches(&ret, i) {
            ret.replace(i, Gate::mux(v[0], v[1], v[2]));
        }
    }
    ret.cleanup();
    ret.make_canonical();
    *aig = ret;
}

fn dffe_pattern() -> Network {
    let mut pattern = Network::new();
    let d = pattern.add_input();
    let en = pattern.add_input();
    let var = Signal::from_var(1);
    let mx = pattern.add(Gate::mux(en, d, var));
    let q = pattern.add(Gate::dff(mx, Signal::one(), Signal::zero()));
    pattern.add_output(q);
    assert_eq!(q, var);
    pattern
}

/// Rebuild Dffe from Mux gates
pub fn infer_dffe(aig: &mut Network) {
    let mut ret = aig.clone();

    let pattern = dffe_pattern();
    let mut matcher = Matcher::from_pattern(&pattern);
    for i in 0..ret.nb_nodes() {
        if let Some(v) = matcher.matches(&ret, i) {
            ret.replace(i, Gate::dff(v[0], v[1], Signal::zero()));
        }
    }
    ret.cleanup();
    ret.make_canonical();
    *aig = ret;
}
