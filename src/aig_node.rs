use std::fmt;

use crate::signal::Signal;

/// Representation of an AIG node
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub struct AigNode {
    a: Signal,
    b: Signal,
    c: Signal,
}
/// Represent the different gates that are supported, including simpler gates
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum Gate {
    And(Signal, Signal),
    Xor(Signal, Signal),
    Mux(Signal, Signal, Signal),
    Maj(Signal, Signal, Signal),
}

/// Represent the core gates that are represented
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum CoreGate {
    Mux(Signal, Signal, Signal),
    Maj(Signal, Signal, Signal),
}

impl AigNode {
    /// Return the input literals, with internal flags removed
    pub fn lits(&self) -> (Signal, Signal, Signal) {
        (
            self.a.without_flag(),
            self.b.without_flag(),
            self.c.without_flag(),
        )
    }

    pub fn maj(a: Signal, b: Signal, c: Signal) -> AigNode {
        AigNode { a, b, c }
    }

    pub fn mux(a: Signal, b: Signal, c: Signal) -> AigNode {
        AigNode {
            a: a.with_flag(),
            b,
            c,
        }
    }

    fn simulate(&self, a_val: u64, b_val: u64, c_val: u64) -> u64 {
        // Convert boolean flags to full-width words
        let toggle_a: u64 = self.a.pol_to_word();
        let toggle_b: u64 = self.b.pol_to_word();
        let toggle_c: u64 = self.c.pol_to_word();
        let sel_mux: u64 = self.a.flag_to_word();
        let av: u64 = a_val ^ toggle_a;
        let bv: u64 = b_val ^ toggle_b;
        let cv: u64 = c_val ^ toggle_c;
        let mux: u64 = (av & bv) | (!av & cv);
        let maj: u64 = (av & bv) | ((av | bv) & cv);
        (!sel_mux & maj) | (sel_mux & mux)
    }
}

impl From<AigNode> for Gate {
    fn from(n: AigNode) -> Gate {
        let (a, b, c) = n.lits();
        if n.a.flag() {
            // Mux/Xor
            if b.pol() && b == !c {
                Gate::Xor(a, c)
            } else {
                Gate::Mux(a, b, c)
            }
        } else if c == Signal::zero() {
            Gate::And(a, b)
        } else {
            Gate::Maj(a, b, c)
        }
    }
}

impl From<AigNode> for CoreGate {
    fn from(n: AigNode) -> CoreGate {
        let (a, b, c) = n.lits();
        if n.a.flag() {
            CoreGate::Mux(a, b, c)
        } else {
            CoreGate::Maj(a, b, c)
        }
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Gate::*;
        match self {
            And(a, b) => {
                write!(f, "And({a}, {b})")
            }
            Xor(a, b) => {
                write!(f, "Xor({a}, {b})")
            }
            Mux(a, b, c) => {
                write!(f, "Mux({a}, {b}, {c})")
            }
            Maj(a, b, c) => {
                write!(f, "Maj({a}, {b}, {c})")
            }
        }
    }
}

impl fmt::Display for CoreGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CoreGate::*;
        match self {
            Mux(a, b, c) => {
                write!(f, "Mux({a}, {b}, {c})")
            }
            Maj(a, b, c) => {
                write!(f, "Maj({a}, {b}, {c})")
            }
        }
    }
}

impl fmt::Display for AigNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&Gate::from(*self), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Typical cases
        assert_eq!(format!("{}", AigNode::maj(i2, i1, i0)), "Maj(v2, v1, v0)");
        assert_eq!(format!("{}", AigNode::maj(i2, i1, l0)), "And(v2, v1)");
        assert_eq!(format!("{}", AigNode::mux(i2, i1, i0)), "Mux(v2, v1, v0)");
        assert_eq!(format!("{}", AigNode::mux(i0, !i1, i1)), "Xor(v0, v1)");

        // No undue simplification when something is not in canonical form
        assert_eq!(format!("{}", AigNode::maj(i2, i1, l1)), "Maj(v2, v1, 1)");
        assert_eq!(format!("{}", AigNode::mux(i0, i1, !i1)), "Mux(v0, v1, !v1)");
    }
}
