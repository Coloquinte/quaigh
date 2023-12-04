use std::{cmp, fmt};

use crate::network::signal::Signal;

/// Basic types of N-input gates
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum NaryType {
    /// N-input And gate
    And,
    /// N-input Or gate
    Or,
    /// N-input Nand gate
    Nand,
    /// N-input Nor gate
    Nor,
    /// N-input Xor gate
    Xor,
    /// N-input Xnor gate
    Xnor,
}

/// Logic gate representation
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Gate {
    /// 2-input And gate
    And(Signal, Signal),
    /// 2-input Xor gate
    Xor(Signal, Signal),
    /// 3-input And gate
    And3(Signal, Signal, Signal),
    /// 3-input Xor gate
    Xor3(Signal, Signal, Signal),
    /// Majority gate
    Maj(Signal, Signal, Signal),
    /// Multiplexer
    Mux(Signal, Signal, Signal),
    /// D flip-flop
    Dff(Signal, Signal, Signal),
    /// N-input gate
    Nary(Box<[Signal]>, NaryType),
    /// Buf or Not
    Buf(Signal),
}

/// Result of normalizing a logic gate
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Normalization {
    Copy(Signal),
    Node(Gate, bool),
}

impl Gate {
    /// Returns whether the gate is in canonical form
    ///
    /// The canonical form is unique, making it easier to simplify and deduplicate
    /// the logic. Inputs and output may be negated, and constant inputs are simplified.
    ///
    /// Canonical form includes:
    ///   * And gates (with optional negated inputs)
    ///   * Xor gates (no negated input)
    ///   * Mux/Maj/Dff
    /// Or/Nor/Nand/Xnor gates are replaced.
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
            Dff(d, en, res) => {
                *en != Signal::zero() && *d != Signal::zero() && *res != Signal::one()
                // TODO: handle synonyms in the inputs resulting in:
                //   * const 0 (en == !d, en == res, res == d)
                //   * remove enable (en == !res)
                //   * remove data (d == res)
            }
            Nary(v, NaryType::And) => sorted_n(v) && v.len() > 3 && !v[0].is_constant(),
            Nary(v, NaryType::Xor) => {
                sorted_n(v) && v.len() > 3 && !v[0].is_constant() && no_inv_n(v)
            }
            Nary(_, _) => false,
            Buf(_) => false,
        }
    }

    /// Obtain the canonical form of the gate
    pub fn make_canonical(&self) -> Normalization {
        use Normalization::*;
        Node(self.clone(), false).make_canonical()
    }

    /// Obtain all signals feeding this gate
    pub fn dependencies(&self) -> Vec<Signal> {
        use Gate::*;
        match self {
            And(a, b) | Xor(a, b) => {
                vec![*a, *b]
            }
            Mux(a, b, c) | And3(a, b, c) | Xor3(a, b, c) | Maj(a, b, c) | Dff(a, b, c) => {
                vec![*a, *b, *c]
            }
            Nary(v, _) => v.clone().into_vec(),
            Buf(s) => vec![*s],
        }
    }

    /// Obtain all internal variables feeding this gate (not inputs or constants)
    pub fn vars(&self) -> Vec<u32> {
        self.dependencies()
            .iter()
            .filter(|s| s.is_var())
            .map(|s| s.var())
            .collect()
    }

    /// Returns whether the gate is combinatorial
    pub fn is_comb(&self) -> bool {
        return !matches!(self, Gate::Dff(_, _, _));
    }

    /// Obtain all internal variables feeding this gate as combinatorial inputs
    pub(crate) fn comb_vars(&self) -> Vec<u32> {
        if self.is_comb() {
            self.vars()
        } else {
            Vec::new()
        }
    }

    /// Apply a remapping of the signals to the gate
    pub(crate) fn remap<F: Fn(&Signal) -> Signal>(&self, t: F) -> Gate {
        use Gate::*;
        match self {
            And(a, b) => And(t(a), t(b)),
            Xor(a, b) => Xor(t(a), t(b)),
            And3(a, b, c) => And3(t(a), t(b), t(c)),
            Xor3(a, b, c) => Xor3(t(a), t(b), t(c)),
            Mux(a, b, c) => Mux(t(a), t(b), t(c)),
            Dff(a, b, c) => Dff(t(a), t(b), t(c)),
            Maj(a, b, c) => Maj(t(a), t(b), t(c)),
            Nary(v, tp) => Nary(v.iter().map(|s| t(s)).collect(), *tp),
            Buf(s) => Buf(t(s)),
        }
    }

    /// Apply a remapping of variable order to the gate
    pub(crate) fn remap_order(&self, t: &[Signal]) -> Gate {
        let f = |s: &Signal| s.remap_order(t);
        self.remap(f)
    }
}

/// Normalize an And
fn make_and(a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let (i0, i1) = sort_2(a, b);
    if i0 == Signal::zero() || i0 == !i1 {
        Copy(Signal::zero() ^ inv)
    } else if i0 == Signal::one() || i0 == i1 {
        Copy(i1 ^ inv)
    } else {
        Node(And(i0, i1), inv)
    }
}

/// Normalize a Xor
fn make_xor(a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let new_inv = a.pol() ^ b.pol() ^ inv;
    let (i0, i1) = sort_2(a.without_pol(), b.without_pol());
    if i0 == Signal::zero() {
        Copy(i1 ^ new_inv)
    } else if i0 == i1 {
        Copy(Signal::from(new_inv))
    } else {
        Node(Xor(i0, i1), new_inv)
    }
}

/// Normalize an And3
fn make_and3(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let (i0, i1, i2) = sort_3(a, b, c);
    if i0 == Signal::zero() || i0 == !i1 || i2 == !i1 {
        Copy(Signal::zero() ^ inv)
    } else if i0 == Signal::one() || i0 == i1 {
        make_and(i1, i2, inv)
    } else if i1 == i2 {
        make_and(i0, i1, inv)
    } else {
        Node(And3(i0, i1, i2), inv)
    }
}

/// Normalize a Xor3
fn make_xor3(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let new_inv = a.pol() ^ b.pol() ^ c.pol() ^ inv;
    let (i0, i1, i2) = sort_3(a.without_pol(), b.without_pol(), c.without_pol());
    if i0 == Signal::zero() {
        make_xor(i1, i2, new_inv)
    } else if i0 == i1 {
        Copy(i2 ^ new_inv)
    } else if i1 == i2 {
        Copy(i0 ^ new_inv)
    } else {
        Node(Xor3(i0, i1, i2), new_inv)
    }
}

/// Normalize a Mux
fn make_mux(s: Signal, a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    if s.pol() {
        make_mux(!s, b, a, inv)
    } else if b.pol() {
        make_mux(s, !a, !b, !inv)
    } else if s == Signal::zero() || a == b {
        Copy(b ^ inv)
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

/// Normalize a Maj
fn make_maj(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let (i0, i1, i2) = sort_3(a, b, c);
    if i0 == !i1 || i1 == i2 {
        Copy(i2 ^ inv)
    } else if i1 == !i2 || i0 == i1 {
        Copy(i0 ^ inv)
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

/// Normalize a Dff
fn make_dff(d: Signal, en: Signal, res: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    if d == Signal::zero() || en == Signal::zero() || res == Signal::one() {
        Copy(Signal::zero() ^ inv)
    } else {
        Node(Dff(d, en, res), inv)
    }
}

/// Normalize a n-ary And
fn make_andn(v: &[Signal], inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let mut vs = v.to_vec();
    vs.retain(|s| *s != Signal::one());
    vs.sort();
    vs.dedup();
    for i in 1..vs.len() {
        if vs[i - 1] == !vs[i] {
            return Copy(Signal::zero() ^ inv);
        }
    }
    if vs.is_empty() {
        Copy(Signal::one() ^ inv)
    } else if vs[0] == Signal::zero() {
        Copy(Signal::zero() ^ inv)
    } else if vs.len() == 1 {
        Copy(vs[0] ^ inv)
    } else if vs.len() == 2 {
        make_and(vs[0], vs[1], inv)
    } else if vs.len() == 3 {
        make_and3(vs[0], vs[1], vs[2], inv)
    } else {
        Node(Nary(vs.into(), NaryType::And), inv)
    }
}

/// Normalize a n-ary Xor
fn make_xorn(v: &[Signal], inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let mut vs = v.to_vec();
    // Remove polarity
    let mut pol = inv;
    for s in vs.iter() {
        pol ^= s.pol();
    }
    for s in &mut vs {
        *s = s.without_pol();
    }
    vs.retain(|s| *s != Signal::zero());
    vs.sort();
    // Remove duplicates
    let mut dd = Vec::new();
    for s in vs {
        if let Some(lst) = dd.last() {
            if *lst != s {
                dd.push(s);
            } else {
                dd.pop();
            }
        } else {
            dd.push(s);
        }
    }
    vs = dd;

    if vs.is_empty() {
        Copy(Signal::zero() ^ pol)
    } else if vs.len() == 1 {
        Copy(vs[0] ^ pol)
    } else if vs.len() == 2 {
        make_xor(vs[0], vs[1], pol)
    } else if vs.len() == 3 {
        make_xor3(vs[0], vs[1], vs[2], pol)
    } else {
        Node(Nary(vs.into(), NaryType::Xor), pol)
    }
}

impl Normalization {
    /// Returns whether the normalization is canonical
    pub fn is_canonical(&self) -> bool {
        use Normalization::*;
        match self {
            Copy(_) => true,
            Node(g, _) => g.is_canonical(),
        }
    }

    /// Apply the normalization algorithm
    pub fn make_canonical(&self) -> Self {
        use Gate::*;
        use Normalization::*;
        match self {
            Copy(s) => Copy(*s),
            Node(g, inv) => match g {
                And(a, b) => make_and(*a, *b, *inv),
                Xor(a, b) => make_xor(*a, *b, *inv),
                And3(a, b, c) => make_and3(*a, *b, *c, *inv),
                Xor3(a, b, c) => make_xor3(*a, *b, *c, *inv),
                Mux(s, a, b) => make_mux(*s, *a, *b, *inv),
                Maj(a, b, c) => make_maj(*a, *b, *c, *inv),
                Dff(d, en, res) => make_dff(*d, *en, *res, *inv),
                Nary(v, t) => {
                    let vi: Box<[Signal]> = v.iter().map(|s| !s).collect();
                    match t {
                        NaryType::And => make_andn(v, *inv),
                        NaryType::Nand => make_andn(v, !inv),
                        NaryType::Xor => make_xorn(v, *inv),
                        NaryType::Xnor => make_xorn(v, !inv),
                        NaryType::Or => make_andn(&vi, !inv),
                        NaryType::Nor => make_andn(&vi, *inv),
                    }
                }
                Buf(s) => Copy(*s ^ *inv),
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
            Nary(v, tp) => {
                let sep = match tp {
                    NaryType::And | NaryType::Nand => " & ",
                    NaryType::Or | NaryType::Nor => " | ",
                    NaryType::Xor | NaryType::Xnor => " ^ ",
                };
                let inv = match tp {
                    NaryType::Nand | NaryType::Nor | NaryType::Xnor => true,
                    _ => false,
                };
                if inv {
                    write!(
                        f,
                        "!({})",
                        v.iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(sep)
                    )
                } else {
                    write!(
                        f,
                        "{}",
                        v.iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(sep)
                    )
                }
            }
            Buf(s) => {
                write!(f, "{}", s)
            }
        }
    }
}

impl fmt::Display for Normalization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Normalization::*;
        match self {
            Copy(s) => write!(f, "{s}"),
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

fn sorted_n(v: &[Signal]) -> bool {
    v.windows(2).all(|w| w[0].ind() < w[1].ind())
}

fn no_inv_2(a: Signal, b: Signal) -> bool {
    !a.pol() && !b.pol()
}

fn no_inv_3(a: Signal, b: Signal, c: Signal) -> bool {
    !a.pol() && !b.pol() && !c.pol()
}

fn no_inv_n(v: &[Signal]) -> bool {
    v.iter().all(|s| !s.pol())
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
        let e0 = Node(n.clone(), false);
        let e1 = Node(n, true);
        let c0 = e0.make_canonical();
        let c1 = e1.make_canonical();
        assert!(c0.is_canonical(), "Canonization is wrong: {e0} to {c0}");
        assert!(c1.is_canonical(), "Canonization is wrong: {e1} to {c1}");

        match (c0, c1) {
            (Copy(s0), Copy(s1)) => assert_eq!(s0, !s1),
            (Node(g0, i0), Node(g1, i1)) => {
                assert_eq!(g0, g1);
                assert_eq!(i0, !i1);
            }
            _ => panic!("Canonization of complements resulted in different gates"),
        }
    }

    #[test]
    fn test_make_canonical() {
        let mut vars = vec![Signal::zero(), Signal::one()];
        for i in 0..4 {
            for b in [false, true] {
                vars.push(Signal::from_ind(i) ^ b);
            }
        }

        for i0 in vars.iter() {
            check_canonization(Buf(*i0));
            for i1 in vars.iter() {
                check_canonization(And(*i0, *i1));
                check_canonization(Xor(*i0, *i1));
                for i2 in vars.iter() {
                    check_canonization(Mux(*i0, *i1, *i2));
                    check_canonization(Maj(*i0, *i1, *i2));
                    check_canonization(And3(*i0, *i1, *i2));
                    check_canonization(Xor3(*i0, *i1, *i2));
                    check_canonization(Dff(*i0, *i1, *i2));
                    for i3 in vars.iter() {
                        check_canonization(Nary(vec![*i0, *i1, *i2, *i3].into(), NaryType::And));
                        check_canonization(Nary(vec![*i0, *i1, *i2, *i3].into(), NaryType::Nand));
                        check_canonization(Nary(vec![*i0, *i1, *i2, *i3].into(), NaryType::Or));
                        check_canonization(Nary(vec![*i0, *i1, *i2, *i3].into(), NaryType::Nor));
                        check_canonization(Nary(vec![*i0, *i1, *i2, *i3].into(), NaryType::Xor));
                        check_canonization(Nary(vec![*i0, *i1, *i2, *i3].into(), NaryType::Xnor));
                    }
                }
            }
        }

        check_canonization(Nary(Vec::new().into(), NaryType::And));
        check_canonization(Nary(Vec::new().into(), NaryType::Nand));
        check_canonization(Nary(Vec::new().into(), NaryType::Or));
        check_canonization(Nary(Vec::new().into(), NaryType::Nor));
        check_canonization(Nary(Vec::new().into(), NaryType::Xor));
        check_canonization(Nary(Vec::new().into(), NaryType::Xnor));
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
