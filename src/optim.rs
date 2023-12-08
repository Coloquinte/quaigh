//! Optimization of logic networks

use crate::{Gate, NaryType, Network, Signal};

/// Merge dependencies of a gate satisfying a given predicate and not inverted. This is used to merge all And/Xor gates.
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
            let g = aig.gate(s.var() as usize);
            let deps = g.dependencies();
            if pred(g) && ret.len() + deps.len() + remaining <= max_size {
                ret.extend(deps);
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

/// Factor And or Xor gates with common inputs
///
/// Transform large gates into trees of binary gates, sharing as many inputs as possible.
pub fn factor_nary(aig: &Network) -> Network {
    aig.clone()
}

#[cfg(test)]
mod tests {
    use crate::{optim::flatten_nary, Gate, NaryType, Network, Signal};

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
