use std::{cmp, fmt};

use crate::signal::Signal;

/// Logic gate
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Gate {
    And(Signal, Signal),
    Xor(Signal, Signal),
    And3(Signal, Signal, Signal),
    Xor3(Signal, Signal, Signal),
    Maj(Signal, Signal, Signal),
    Mux(Signal, Signal, Signal),
    Dff(Signal, Signal, Signal),
}

/// Result of normalizing a logic gate
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Normalization {
    Buf(Signal),
    Node(Gate, bool),
}

impl Gate {
    pub fn is_canonical(&self) -> bool {
        use Gate::*;
        match self {
            And(a, b) => sorted_2(*a, *b) && !a.is_constant(),
            Xor(a, b) => sorted_2(*a, *b) && !a.is_constant() && no_inv_2(*a, *b),
            And3(a, b, c) => sorted_3(*a, *b, *c) && !a.is_constant(),
            Xor3(a, b, c) => sorted_3(*a, *b, *c) && !a.is_constant() && no_inv_3(*a, *b, *c),
            Maj(a, b, c) => sorted_3(*a, *b, *c) && !a.is_constant() && !a.pol(),
            Mux(s, a, b) => {
                s.ind() != a.ind()
                    && s.ind() != b.ind()
                    && a.ind() != b.ind()
                    && !s.pol()
                    && !b.pol()
                    && !a.is_constant()
                    && !b.is_constant()
                    && !s.is_constant()
            }
            Dff(d, en, r_es) => *en != Signal::zero() && *d != Signal::zero(),
        }
    }

    pub fn make_canonical(&self) -> Normalization {
        use Normalization::*;
        Node(*self, false).make_canonical()
    }
}

fn make_and(a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let (i0, i1) = sort_2(a, b);
    if i0 == Signal::zero() || i0 == !i1 {
        Buf(Signal::zero() ^ inv)
    } else if i0 == Signal::one() || i0 == i1 {
        Buf(i1 ^ inv)
    } else {
        Node(And(i0, i1), inv)
    }
}

fn make_xor(a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let new_inv = a.pol() ^ b.pol() ^ inv;
    let (i0, i1) = sort_2(a.without_pol(), b.without_pol());
    if i0 == Signal::zero() {
        Buf(i1 ^ new_inv)
    } else if i0 == i1 {
        Buf(Signal::from(new_inv))
    } else {
        Node(Xor(i0, i1), new_inv)
    }
}

fn make_and3(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let (i0, i1, i2) = sort_3(a, b, c);
    if i0 == Signal::zero() || i0 == !i1 || i2 == !i1 {
        Buf(Signal::zero() ^ inv)
    } else if i0 == Signal::one() || i0 == i1 {
        make_and(i1, i2, inv)
    } else if i1 == i2 {
        make_and(i0, i1, inv)
    } else {
        Node(And3(i0, i1, i2), inv)
    }
}

fn make_xor3(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let new_inv = a.pol() ^ b.pol() ^ c.pol() ^ inv;
    let (i0, i1, i2) = sort_3(a.without_pol(), b.without_pol(), c.without_pol());
    if i0 == Signal::zero() {
        make_xor(i1, i2, new_inv)
    } else if i0 == i1 {
        Buf(i2 ^ new_inv)
    } else if i1 == i2 {
        Buf(i0 ^ new_inv)
    } else {
        Node(Xor3(i0, i1, i2), new_inv)
    }
}

fn make_mux(s: Signal, a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    if s.pol() {
        make_mux(!s, b, a, inv)
    } else if b.pol() {
        make_mux(s, !a, !b, !inv)
    } else if s == Signal::zero() || a == b {
        Buf(b ^ inv)
    } else if s == a || a == Signal::one() {
        // s ? 1 : b ==> s | b ==> !(!s & !b)
        make_and(!s, !b, !inv)
    } else if s == !a || a == Signal::zero() {
        // s ? 0 : b ==> !s & b
        make_and(!s, b, inv)
    } else if s == b || b == Signal::zero() {
        // s ? a : 0 ==> s & a
        make_and(s, a, inv)
    } else if s == !b || b == Signal::one() {
        // s ? a : 1 ==> !s | a ==> !(s & !a)
        make_and(s, !a, !inv)
    } else if a == !b {
        // s ? !b : b ==> s ^ b
        make_xor(s, b, inv)
    } else {
        Node(Mux(s, a, b), inv)
    }
}

fn make_maj(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let (i0, i1, i2) = sort_3(a, b, c);
    if i0 == !i1 || i1 == i2 {
        Buf(i2 ^ inv)
    } else if i1 == !i2 || i0 == i1 {
        Buf(i0 ^ inv)
    } else if i0.pol() {
        // Won't cause an infinite loop because the order will not change
        // We already removed cases where signals differ by their sign
        make_maj(!i0, !i1, !i2, !inv)
    } else if i0 == Signal::zero() {
        make_and(i1, i2, inv)
    } else {
        Node(Maj(i0, i1, i2), inv)
    }
}

fn make_dff(d: Signal, en: Signal, res: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    if d == Signal::zero() || en == Signal::zero() {
        Buf(Signal::zero() ^ inv)
    } else {
        Node(Dff(d, en, res), inv)
    }
}

impl Normalization {
    pub fn is_canonical(&self) -> bool {
        use Normalization::*;
        match self {
            Buf(_) => true,
            Node(g, _) => g.is_canonical(),
        }
    }

    pub fn make_canonical(&self) -> Self {
        use Gate::*;
        use Normalization::*;
        match self {
            Buf(s) => Buf(*s),
            Node(g, inv) => match g {
                And(a, b) => make_and(*a, *b, *inv),
                Xor(a, b) => make_xor(*a, *b, *inv),
                And3(a, b, c) => make_and3(*a, *b, *c, *inv),
                Xor3(a, b, c) => make_xor3(*a, *b, *c, *inv),
                Mux(s, a, b) => make_mux(*s, *a, *b, *inv),
                Maj(a, b, c) => make_maj(*a, *b, *c, *inv),
                Dff(d, en, res) => make_dff(*d, *en, *res, *inv),
            },
        }
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Gate::*;
        match self {
            And(a, b) => {
                write!(f, "{a} & {b}")
            }
            Xor(a, b) => {
                write!(f, "{a} ^ {b}")
            }
            And3(a, b, c) => {
                write!(f, "{a} & {b} & {c}")
            }
            Xor3(a, b, c) => {
                write!(f, "{a} ^ {b} ^ {c}")
            }
            Mux(s, a, b) => {
                write!(f, "{s} ? {a} : {b}")
            }
            Maj(a, b, c) => {
                write!(f, "Maj({a}, {b}, {c})")
            }
            Dff(d, en, res) => {
                write!(f, "Dff({d}")?;
                if *en != Signal::one() {
                    write!(f, ", en={en}")?;
                }
                if *res != Signal::zero() {
                    write!(f, ", res={res}")?;
                }
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for Normalization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Normalization::*;
        match self {
            Buf(s) => write!(f, "{s}"),
            Node(g, inv) => {
                if *inv {
                    write!(f, "!(")?;
                }
                write!(f, "{g}")?;
                if *inv {
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

fn sorted_2(a: Signal, b: Signal) -> bool {
    a.ind() < b.ind()
}

fn sorted_3(a: Signal, b: Signal, c: Signal) -> bool {
    a.ind() < b.ind() && b.ind() < c.ind()
}

fn no_inv_2(a: Signal, b: Signal) -> bool {
    !a.pol() && !b.pol()
}

fn no_inv_3(a: Signal, b: Signal, c: Signal) -> bool {
    !a.pol() && !b.pol() && !c.pol()
}

fn sort_2(a: Signal, b: Signal) -> (Signal, Signal) {
    (cmp::min(a, b), cmp::max(a, b))
}

fn sort_3(a: Signal, b: Signal, c: Signal) -> (Signal, Signal, Signal) {
    let (mut i0, mut i1, mut i2) = (a, b, c);
    (i1, i2) = sort_2(i1, i2);
    (i0, i1) = sort_2(i0, i1);
    (i1, i2) = sort_2(i1, i2);
    (i0, i1, i2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Gate::*;
    use Normalization::*;

    fn check_canonization(n: Gate) {
        let e0 = Node(n, false);
        let e1 = Node(n, true);
        let c0 = e0.make_canonical();
        let c1 = e1.make_canonical();
        assert!(c0.is_canonical(), "Canonization is wrong: {e0} to {c0}");
        assert!(c1.is_canonical(), "Canonization is wrong: {e1} to {c1}");

        match (c0, c1) {
            (Buf(s0), Buf(s1)) => assert_eq!(s0, !s1),
            (Node(g0, i0), Node(g1, i1)) => {
                assert_eq!(g0, g1);
                assert_eq!(i0, !i1);
            }
            _ => panic!("Canonization of complements resulted in different gates"),
        }
    }

    #[test]
    fn test_make_canonical() {
        let mut vars = vec![];
        for i in 0..4 {
            for b in [false, true] {
                vars.push(Signal::from_ind(i) ^ b);
            }
        }

        for i0 in vars.iter() {
            for i1 in vars.iter() {
                check_canonization(And(*i0, *i1));
                check_canonization(Xor(*i0, *i1));
                for i2 in vars.iter() {
                    check_canonization(Mux(*i0, *i1, *i2));
                    check_canonization(Maj(*i0, *i1, *i2));
                    check_canonization(And3(*i0, *i1, *i2));
                    check_canonization(Xor3(*i0, *i1, *i2));
                    check_canonization(Dff(*i0, *i1, *i2));
                }
            }
        }
    }

    #[test]
    fn test_and_is_canonical() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);

        // Everything OK
        assert!(And(i0, i1).is_canonical());
        assert!(And(i0, !i1).is_canonical());
        assert!(And(!i0, i1).is_canonical());
        assert!(And(!i0, !i1).is_canonical());

        // Wrong ordering
        assert!(!And(i1, i0).is_canonical());
        assert!(!And(i1, !i0).is_canonical());
        assert!(!And(!i1, i0).is_canonical());
        assert!(!And(!i1, !i0).is_canonical());

        // Constant
        assert!(!And(l0, i1).is_canonical());
        assert!(!And(l1, i1).is_canonical());

        // Repeatition
        assert!(!And(i0, i0).is_canonical());
        assert!(!And(i0, !i0).is_canonical());
    }

    #[test]
    fn test_xor_is_canonical() {
        let l0 = Signal::zero();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);

        // Everything OK
        assert!(Xor(i0, i1).is_canonical());

        // Wrong ordering
        assert!(!Xor(i1, i0).is_canonical());

        // Bad polarity
        assert!(!Xor(i0, !i1).is_canonical());
        assert!(!Xor(!i0, i1).is_canonical());

        // Constant
        assert!(!Xor(l0, i1).is_canonical());

        // Repeatition
        assert!(!Xor(i0, i0).is_canonical());
    }

    #[test]
    fn test_maj_is_canonical() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Everything OK
        assert!(Maj(i0, i1, i2).is_canonical());
        assert!(Maj(i0, !i1, i2).is_canonical());
        assert!(Maj(i0, !i1, i2).is_canonical());
        assert!(Maj(i0, !i1, !i2).is_canonical());

        // Wrong ordering
        assert!(!Maj(i0, i2, i1).is_canonical());
        assert!(!Maj(i1, i0, i2).is_canonical());

        // Constant
        assert!(!Maj(l0, i1, i2).is_canonical());
        assert!(!Maj(l1, i1, i2).is_canonical());

        // Wrong polarity
        assert!(!Maj(!i0, i1, i2).is_canonical());
        assert!(!Maj(!i0, !i1, i2).is_canonical());
        assert!(!Maj(!i0, !i1, i2).is_canonical());
        assert!(!Maj(!i0, !i1, !i2).is_canonical());

        // Repeatition
        assert!(!Maj(i0, i0, i2).is_canonical());
        assert!(!Maj(i0, !i0, i2).is_canonical());
        assert!(!Maj(i0, i2, i2).is_canonical());
        assert!(!Maj(i0, i2, !i2).is_canonical());
    }

    #[test]
    fn test_mux_is_canonical() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Everything OK
        assert!(Mux(i2, i1, i0).is_canonical());
        assert!(Mux(i2, !i1, i0).is_canonical());

        // Bad polarity
        assert!(!Mux(i2, i1, !i0).is_canonical());
        assert!(!Mux(i2, !i1, !i0).is_canonical());
        assert!(!Mux(!i2, i1, i0).is_canonical());
        assert!(!Mux(!i2, !i1, i0).is_canonical());

        // Constant anywhere
        assert!(!Mux(l0, i1, i0).is_canonical());
        assert!(!Mux(i2, l0, i0).is_canonical());
        assert!(!Mux(i2, i1, l0).is_canonical());
        assert!(!Mux(i2, i1, !l0).is_canonical());
        assert!(!Mux(l1, i1, i0).is_canonical());
        assert!(!Mux(i2, l1, i0).is_canonical());
        assert!(!Mux(i2, i1, l1).is_canonical());
        assert!(!Mux(i2, i1, !l1).is_canonical());

        // Repeatition anywhere
        assert!(!Mux(i2, i2, i0).is_canonical());
        assert!(!Mux(i0, i2, i2).is_canonical());
        assert!(!Mux(i2, i0, i2).is_canonical());
        assert!(!Mux(i2, !i2, i0).is_canonical());
        assert!(!Mux(i0, i2, !i2).is_canonical());
        assert!(!Mux(i2, i0, !i2).is_canonical());
        assert!(!Mux(!i2, i2, i0).is_canonical());
        assert!(!Mux(i0, !i2, i2).is_canonical());
        assert!(!Mux(!i2, i0, i2).is_canonical());
    }
}
