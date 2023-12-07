//! Network generators and templates

/// Adder generators
pub mod adder {
    use crate::{Gate, Network, Signal};

    /// A simple and slow ripple-carry adder
    pub fn ripple_carry(len: usize) -> Network {
        let mut ret = Network::new();
        let mut c = Signal::zero();
        for _ in 0..len {
            let a = ret.add_input();
            let b = ret.add_input();
            let next_c = ret.add(Gate::Maj(a, b, c));
            let o = ret.add(Gate::Xor3(a, b, c));
            ret.add_output(o);
            c = next_c;
        }
        ret.add_output(c);
        ret.check();
        ret
    }
}

/// Carry chain generators
pub mod carry_chain {
    use crate::{Network, Signal};

    /// A simple and slow ripple-carry chain
    pub fn ripple_carry(len: usize) -> Network {
        let mut ret = Network::new();
        let mut c = Signal::zero();
        for _ in 0..len {
            let propagate = ret.add_input();
            let generate = ret.add_input();
            let d = ret.and(propagate, c);
            c = !ret.and(!generate, !d);
            ret.add_output(c);
        }
        ret
    }
}

/// Simple generators to test functionality
pub mod testcases {
    use crate::{Network, Signal};

    /// A circular chain of Dffs with a Xor with input at the start; used to test topological sorting
    pub fn toggle_chain(len: usize, has_en: bool, has_res: bool) -> Network {
        assert!(len > 0);
        let mut ret = Network::new();
        let input = ret.add_input();
        let en = if has_en {
            ret.add_input()
        } else {
            Signal::one()
        };
        let res = if has_res {
            ret.add_input()
        } else {
            Signal::zero()
        };
        let xor_output = Signal::from_var(len as u32);
        let mut x = input;
        for _ in 0..len {
            x = ret.dff(x, en, res);
        }
        x = ret.xor(x, input);
        ret.add_output(x);
        assert_eq!(x, xor_output);
        ret.check();
        assert!(ret.is_topo_sorted());
        ret
    }

    /// An expanding tree of Dffs, used to test deduplication
    pub fn ff_tree(depth: usize, has_en: bool, has_res: bool, expansion: usize) -> Network {
        let mut ret = Network::new();
        let input = ret.add_input();
        let en = if has_en {
            ret.add_input()
        } else {
            Signal::one()
        };
        let res = if has_res {
            ret.add_input()
        } else {
            Signal::zero()
        };
        let mut stage = vec![input];
        for _ in 0..depth {
            let mut next_stage = Vec::new();
            for s in stage {
                for _ in 0..expansion {
                    next_stage.push(ret.dff(s, en, res));
                }
            }
            stage = next_stage;
        }
        for s in stage {
            ret.add_output(s);
        }
        ret.check();
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::{adder, carry_chain, testcases};

    #[test]
    fn test_adder() {
        for i in [0, 1, 2, 4, 8, 16, 32, 64, 128] {
            adder::ripple_carry(i);
        }
    }

    #[test]
    fn test_carry_chain() {
        for i in [0, 1, 2, 4, 8, 16, 32, 64, 128] {
            carry_chain::ripple_carry(i);
        }
    }

    #[test]
    fn test_toggle_chain() {
        for i in [1, 2, 4, 8, 16, 32, 64, 128] {
            for has_en in [false, true] {
                for has_res in [false, true] {
                    let mut aig = testcases::toggle_chain(i, has_en, has_res);
                    assert_eq!(aig.nb_nodes(), i + 1);
                    aig.sweep();
                    aig.dedup();
                    assert_eq!(aig.nb_nodes(), i + 1);
                }
            }
        }
    }

    #[test]
    fn test_ff_tree() {
        for i in [0, 1, 2, 3, 4, 5] {
            for expansion in [1, 2] {
                for has_en in [false, true] {
                    for has_res in [false, true] {
                        let mut aig = testcases::ff_tree(i, has_en, has_res, expansion);
                        aig.sweep();
                        for _ in 0..i {
                            // Run dedup several times, once per Dff level
                            aig.dedup();
                        }
                        assert_eq!(aig.nb_nodes(), i);
                    }
                }
            }
        }
    }
}
