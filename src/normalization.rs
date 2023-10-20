use std::cmp;

use crate::signal::Signal;

#[derive(Debug)]
pub enum Normalization {
    Buf(Signal, bool),
    Mux(Signal, Signal, Signal, bool),
    Maj(Signal, Signal, Signal, bool),
}
use Normalization::*;

impl Normalization {
    pub fn is_canonical(&self) -> bool {
        match self {
            Buf(_, inv) => !inv,
            Maj(a, b, c, _) => {
                if a <= b || b <= c {
                    return false;
                }
                if a.ind() == b.ind() || b.ind() == c.ind() {
                    return false;
                }
                if c.pol() {
                    return false;
                }
                true
            }
            Mux(s, a, b, _) => {
                if s.pol() || b.pol() {
                    // No polarity on selector nor second input
                    return false;
                }
                if s.is_constant() || a.is_constant() || b.is_constant() {
                    // No constant input
                    return false;
                }
                if a == b || s.ind() == a.ind() || s.ind() == b.ind() {
                    // No repeated input except b == !a
                    return false;
                }
                if a.ind() == b.ind() && s < a {
                    // For a xor, selector is always bigger
                    return false;
                }
                true
            }
        }
    }

    pub fn make_canonical(&self) -> Self {
        match self {
            Buf(l, inv) => Buf(l ^ *inv, false),
            Maj(a, b, c, inv) => {
                let (i2, i1, i0) = sort_3_lits((*a, *b, *c));
                if i2 == i1 || i1 == i0 {
                    return Buf(i1, *inv).make_canonical();
                }
                if i2 == !i1 {
                    return Buf(i0, *inv).make_canonical();
                }
                if i0 == !i1 {
                    return Buf(i2, *inv).make_canonical();
                }
                if i0.pol() {
                    Maj(!i2, !i1, !i0, !inv)
                } else {
                    Maj(i2, i1, i0, *inv)
                }
            }
            Mux(s, a, b, inv) => {
                if s.pol() {
                    return Mux(!*s, *b, *a, *inv).make_canonical();
                }
                debug_assert!(!s.pol());
                if b.pol() {
                    return Mux(*s, !*a, !*b, !*inv).make_canonical();
                }
                debug_assert!(!b.pol());
                if *s == Signal::zero() || a == b {
                    return Buf(*b, *inv).make_canonical();
                }
                debug_assert!(!s.is_constant());
                debug_assert!(a != b);
                if *s == *a || *a == Signal::one() {
                    // s ? 1 : b ==> s | b ==> !(!s & !b)
                    return Maj(!*s, !*b, Signal::zero(), !*inv).make_canonical();
                }
                if *s == !*a || *a == Signal::zero() {
                    // s ? 0 : b ==> !s & b
                    return Maj(!*s, *b, Signal::zero(), *inv).make_canonical();
                }
                if *s == *b || *b == Signal::zero() {
                    // s ? a : 0 ==> s & a
                    return Maj(*s, *a, Signal::zero(), *inv).make_canonical();
                }
                if *s == !*b || *b == Signal::one() {
                    // s ? a : 1 ==> !s | a ==> !(s & !a)
                    return Maj(*s, !*a, Signal::zero(), !*inv).make_canonical();
                }
                debug_assert!(!a.is_constant());
                debug_assert!(!b.is_constant());
                if *a == !*b && *s < *a {
                    // s ^ b, but need to swap s and b
                    return Mux(*b, !*s, *s, *inv);
                }
                Mux(*s, *a, *b, *inv)
            }
        }
    }
}

fn sort_2_lits(lits: (Signal, Signal)) -> (Signal, Signal) {
    let (i1, i0) = lits;
    (cmp::max(i1, i0), cmp::min(i1, i0))
}

fn sort_3_lits(lits: (Signal, Signal, Signal)) -> (Signal, Signal, Signal) {
    let (mut i2, mut i1, mut i0) = lits;
    (i2, i1) = sort_2_lits((i2, i1));
    (i1, i0) = sort_2_lits((i1, i0));
    (i2, i1) = sort_2_lits((i2, i1));
    (i2, i1, i0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mux(s: Signal, a: Signal, b: Signal) -> Normalization {
        Mux(s, a, b, false)
    }

    fn maj(a: Signal, b: Signal, c: Signal) -> Normalization {
        Maj(a, b, c, false)
    }

    #[test]
    fn test_maj_is_canonical() {
        let l0 = Signal::zero();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Everything OK
        assert!(maj(i2, i1, i0).is_canonical());
        assert!(maj(i2, !i1, i0).is_canonical());
        assert!(maj(!i2, !i1, i0).is_canonical());
        assert!(maj(!i2, i1, i0).is_canonical());
        assert!(maj(i1, i0, l0).is_canonical());
        assert!(maj(i1, !i0, l0).is_canonical());
        assert!(maj(!i1, !i0, l0).is_canonical());
        assert!(maj(!i1, i0, l0).is_canonical());

        // Wrong ordering
        assert!(!maj(i2, i0, i1).is_canonical());
        assert!(!maj(i1, i2, i0).is_canonical());
        assert!(!maj(i2, l0, i1).is_canonical());
        assert!(!maj(l0, i2, i0).is_canonical());

        // Duplication
        assert!(!maj(i2, i1, i1).is_canonical());
        assert!(!maj(i1, i1, i0).is_canonical());
        assert!(!maj(i2, !i1, i1).is_canonical());
        assert!(!maj(!i1, i1, i0).is_canonical());

        // Wrong polarity
        assert!(!maj(i2, i1, !i0).is_canonical());
        assert!(!maj(i2, !i1, !i0).is_canonical());
        assert!(!maj(!i2, !i1, !i0).is_canonical());
        assert!(!maj(!i2, i1, !i0).is_canonical());
        assert!(!maj(i1, i0, !l0).is_canonical());
        assert!(!maj(i1, !i0, !l0).is_canonical());
        assert!(!maj(!i1, !i0, !l0).is_canonical());
        assert!(!maj(!i1, i0, !l0).is_canonical());
    }

    #[test]
    fn test_maj_make_canonical() {
        let mut vars = vec![];
        for i in 0..4 {
            for b in [false, true] {
                vars.push(Signal::from_ind(i) ^ b);
            }
        }

        for i0 in vars.iter() {
            for i1 in vars.iter() {
                for i2 in vars.iter() {
                    let exp = maj(*i0, *i1, *i2);
                    let can = exp.make_canonical();
                    assert!(
                        can.is_canonical(),
                        "Canonization is wrong: {exp:?} to {can:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_mux_is_canonical() {
        let l0 = Signal::zero();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Everything OK
        assert!(mux(i2, i1, i0).is_canonical());
        assert!(mux(i2, !i1, i0).is_canonical());
        assert!(mux(i2, !i0, i0).is_canonical());

        // Bad polarity
        assert!(!mux(i2, i1, !i0).is_canonical());
        assert!(!mux(i2, !i1, !i0).is_canonical());
        assert!(!mux(!i2, i1, i0).is_canonical());
        assert!(!mux(!i2, !i1, i0).is_canonical());

        // Constant anywhere
        assert!(!mux(l0, i1, i0).is_canonical());
        assert!(!mux(i2, l0, i0).is_canonical());
        assert!(!mux(i2, i1, l0).is_canonical());
        assert!(!mux(i2, i1, !l0).is_canonical());

        // Repeatition
        assert!(!mux(i2, i2, i0).is_canonical());
        assert!(!mux(i2, i0, i0).is_canonical());
        assert!(!mux(i2, i0, i2).is_canonical());
        assert!(!mux(i2, !i2, i0).is_canonical());
        assert!(!mux(i2, i0, !i2).is_canonical());

        // Xor in wrong order
        assert!(!mux(i0, !i2, i2).is_canonical());
    }

    #[test]
    fn test_mux_make_canonical() {
        let mut vars = vec![];
        for i in 0..4 {
            for b in [false, true] {
                vars.push(Signal::from_ind(i) ^ b);
            }
        }

        for i0 in vars.iter() {
            for i1 in vars.iter() {
                for i2 in vars.iter() {
                    let exp = mux(*i0, *i1, *i2);
                    let can = exp.make_canonical();
                    assert!(
                        can.is_canonical(),
                        "Canonization is wrong: {exp:?} to {can:?}"
                    );
                }
            }
        }
    }
}
