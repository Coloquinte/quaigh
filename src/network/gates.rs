use core::slice;
use std::{cmp, fmt};

use volute::Lut;

use crate::network::signal::Signal;

/// Basic types of 2-input gates
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum BinaryType {
    /// 2-input And gate
    And,
    /// 2-input Xor gate
    Xor,
}

/// Basic types of 3-input gates
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum TernaryType {
    /// 3-input And gate
    And,
    /// 3-input Xor gate
    Xor,
    /// Majority gate (a + b + c >= 2)
    Maj,
    /// Multiplexer a ? b : c
    Mux,
}

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

/// Lut gate
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct LutGate {
    pub inputs: Box<[Signal]>,
    pub lut: Lut,
}

/// Logic gate representation
///
/// Logic gates have a canonical form.
/// The canonical form is unique, making it easier to simplify and deduplicate
/// the logic. Inputs and output may be negated, and constant inputs are simplified.
///
/// Canonical form includes:
///   * And gates (with optional negated inputs)
///   * Xor gates (no negated input)
///   * Mux/Maj/Dff
/// Or/Nor/Nand gates are replaced by And gates.
/// Xnor gates are replaced by Xor gates.
/// Buf/Not and trivial gates are omitted.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Gate {
    /// Arbitrary 2-input gate (And/Xor)
    Binary([Signal; 2], BinaryType),
    /// Arbitrary 3-input gate (And/Xor/Mux/Maj)
    Ternary([Signal; 3], TernaryType),
    /// Arbitrary N-input gate (And/Or/Xor/Nand/Nor/Xnor)
    Nary(Box<[Signal]>, NaryType),
    /// Buf or Not
    Buf(Signal),
    /// D flip-flop with enable and reset
    Dff([Signal; 3]),
    /// LUT
    Lut(Box<LutGate>),
}

/// Result of normalizing a logic gate
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Normalization {
    /// A gate, with an optional inverted output
    Node(Gate, bool),
    /// The trivial case, where the gate reduces to a single signal or constant
    Copy(Signal),
}

impl Gate {
    /// Create a 2-input And
    pub fn and(a: Signal, b: Signal) -> Gate {
        Gate::Binary([a, b], BinaryType::And)
    }

    /// Create a 2-input Xor
    pub fn xor(a: Signal, b: Signal) -> Gate {
        Gate::Binary([a, b], BinaryType::Xor)
    }

    /// Create a 3-input And
    pub fn and3(a: Signal, b: Signal, c: Signal) -> Gate {
        Gate::Ternary([a, b, c], TernaryType::And)
    }

    /// Create a 3-input Xor
    pub fn xor3(a: Signal, b: Signal, c: Signal) -> Gate {
        Gate::Ternary([a, b, c], TernaryType::Xor)
    }

    /// Create a n-input And
    pub fn andn(v: &[Signal]) -> Gate {
        Gate::Nary(v.into(), NaryType::And)
    }

    /// Create a n-input Xor
    pub fn xorn(v: &[Signal]) -> Gate {
        Gate::Nary(v.into(), NaryType::Xor)
    }

    /// Create a n-input Lut
    pub fn lut(v: &[Signal], lut: Lut) -> Gate {
        Gate::Lut(Box::new(LutGate {
            inputs: v.into(),
            lut,
        }))
    }

    /// Create a Mux
    pub fn mux(s: Signal, a: Signal, b: Signal) -> Gate {
        Gate::Ternary([s, a, b], TernaryType::Mux)
    }

    /// Create a Maj
    pub fn maj(a: Signal, b: Signal, c: Signal) -> Gate {
        Gate::Ternary([a, b, c], TernaryType::Maj)
    }

    /// Create a Dff
    pub fn dff(d: Signal, en: Signal, res: Signal) -> Gate {
        Gate::Dff([d, en, res])
    }

    /// Returns whether the gate is in canonical form
    pub fn is_canonical(&self) -> bool {
        use Gate::*;
        match self {
            Binary([a, b], BinaryType::And) => sorted_2(*a, *b) && !a.is_constant(),
            Binary([a, b], BinaryType::Xor) => {
                sorted_2(*a, *b) && !a.is_constant() && no_inv_2(*a, *b)
            }
            Ternary([a, b, c], TernaryType::And) => sorted_3(*a, *b, *c) && !a.is_constant(),
            Ternary([a, b, c], TernaryType::Xor) => {
                sorted_3(*a, *b, *c) && !a.is_constant() && no_inv_3(*a, *b, *c)
            }
            Ternary([a, b, c], TernaryType::Maj) => {
                sorted_3(*a, *b, *c) && !a.is_constant() && !a.is_inverted()
            }
            Ternary([s, a, b], TernaryType::Mux) => {
                s.ind() != a.ind()
                    && s.ind() != b.ind()
                    && a.ind() != b.ind()
                    && !s.is_inverted()
                    && !b.is_inverted()
                    && !a.is_constant()
                    && !b.is_constant()
                    && !s.is_constant()
            }
            Nary(v, NaryType::And) => sorted_n(v) && v.len() > 3 && !v[0].is_constant(),
            Nary(v, NaryType::Xor) => {
                sorted_n(v) && v.len() > 3 && !v[0].is_constant() && no_inv_n(v)
            }
            Nary(_, _) => false,
            Dff([d, en, res]) => {
                *en != Signal::zero() && *d != Signal::zero() && *res != Signal::one()
                // TODO: handle synonyms in the inputs resulting in:
                //   * const 0 (en == !d, en == res, res == d)
                //   * remove enable (en == !res)
                //   * remove data (d == res)
            }
            Buf(_) => false,
            Lut(_) => true,
        }
    }

    /// Obtain the canonical form of the gate
    pub fn make_canonical(&self) -> Normalization {
        use Normalization::*;
        Node(self.clone(), false).make_canonical()
    }

    /// Obtain all signals feeding this gate
    pub fn dependencies(&self) -> &[Signal] {
        use Gate::*;
        match self {
            Binary(s, _) => s,
            Ternary(s, _) => s,
            Nary(v, _) => v,
            Dff(s) => s,
            Buf(s) => slice::from_ref(s),
            Lut(lut) => lut.inputs.as_ref(),
        }
    }

    /// Obtain all internal variables feeding this gate (not inputs or constants)
    pub fn vars(&self) -> impl Iterator<Item = u32> + '_ {
        // TODO: return a concrete iterator instead
        self.dependencies()
            .iter()
            .filter(|s| s.is_var())
            .map(|s| s.var())
    }

    /// Returns whether the gate is combinatorial
    pub fn is_comb(&self) -> bool {
        !matches!(self, Gate::Dff(_))
    }

    /// Returns whether the gate is an And of any arity
    pub fn is_and(&self) -> bool {
        matches!(
            self,
            Gate::Binary(_, BinaryType::And)
                | Gate::Ternary(_, TernaryType::And)
                | Gate::Nary(_, NaryType::And)
        )
    }

    /// Returns whether the gate is a Xor of any arity
    pub fn is_xor(&self) -> bool {
        matches!(
            self,
            Gate::Binary(_, BinaryType::Xor)
                | Gate::Ternary(_, TernaryType::Xor)
                | Gate::Nary(_, NaryType::Xor)
        )
    }

    /// Returns whether the gate is an And, Or, Nand or Nor of any arity
    pub fn is_and_like(&self) -> bool {
        matches!(
            self,
            Gate::Binary(_, BinaryType::And)
                | Gate::Ternary(_, TernaryType::And)
                | Gate::Nary(_, NaryType::And)
                | Gate::Nary(_, NaryType::Nand)
                | Gate::Nary(_, NaryType::Or)
                | Gate::Nary(_, NaryType::Nor)
        )
    }

    /// Returns whether the gate is a Xor, Xnor of any arity
    pub fn is_xor_like(&self) -> bool {
        matches!(
            self,
            Gate::Binary(_, BinaryType::Xor)
                | Gate::Ternary(_, TernaryType::Xor)
                | Gate::Nary(_, NaryType::Xor)
                | Gate::Nary(_, NaryType::Xnor)
        )
    }

    /// Returns whether the gate is a Buf
    pub fn is_buf_like(&self) -> bool {
        matches!(self, Gate::Buf(_))
    }

    /// Apply a remapping of the signals to the gate
    pub(crate) fn remap<F: Fn(&Signal) -> Signal>(&self, t: F) -> Gate {
        use Gate::*;
        match self {
            Binary([a, b], tp) => Binary([t(a), t(b)], *tp),
            Ternary([a, b, c], tp) => Ternary([t(a), t(b), t(c)], *tp),
            Dff([a, b, c]) => Dff([t(a), t(b), t(c)]),
            Nary(v, tp) => Nary(v.iter().map(&t).collect(), *tp),
            Buf(s) => Buf(t(s)),
            Lut(lut) => Lut(Box::new(LutGate {
                inputs: lut.inputs.iter().map(t).collect(),
                lut: lut.lut.clone(),
            })),
        }
    }

    /// Apply a remapping of the signals to the gate that takes the position as argument
    pub(crate) fn remap_with_ind<F: Fn(&Signal, usize) -> Signal>(&self, t: F) -> Gate {
        use Gate::*;
        match self {
            Binary([a, b], tp) => Binary([t(a, 0), t(b, 1)], *tp),
            Ternary([a, b, c], tp) => Ternary([t(a, 0), t(b, 1), t(c, 2)], *tp),
            Dff([a, b, c]) => Dff([t(a, 0), t(b, 1), t(c, 2)]),
            Nary(v, tp) => Nary(v.iter().enumerate().map(|(i, s)| t(s, i)).collect(), *tp),
            Buf(s) => Buf(t(s, 0)),
            Lut(lut) => Lut(Box::new(LutGate {
                inputs: lut
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(i, s)| t(s, i))
                    .collect(),
                lut: lut.lut.clone(),
            })),
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
        Node(Binary([i0, i1], BinaryType::And), inv)
    }
}

/// Normalize a Xor
fn make_xor(a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let new_inv = a.is_inverted() ^ b.is_inverted() ^ inv;
    let (i0, i1) = sort_2(a.without_inversion(), b.without_inversion());
    if i0 == Signal::zero() {
        Copy(i1 ^ new_inv)
    } else if i0 == i1 {
        Copy(Signal::from(new_inv))
    } else {
        Node(Binary([i0, i1], BinaryType::Xor), new_inv)
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
        Node(Ternary([i0, i1, i2], TernaryType::And), inv)
    }
}

/// Normalize a Xor3
fn make_xor3(a: Signal, b: Signal, c: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    let new_inv = a.is_inverted() ^ b.is_inverted() ^ c.is_inverted() ^ inv;
    let (i0, i1, i2) = sort_3(
        a.without_inversion(),
        b.without_inversion(),
        c.without_inversion(),
    );
    if i0 == Signal::zero() {
        make_xor(i1, i2, new_inv)
    } else if i0 == i1 {
        Copy(i2 ^ new_inv)
    } else if i1 == i2 {
        Copy(i0 ^ new_inv)
    } else {
        Node(Ternary([i0, i1, i2], TernaryType::Xor), new_inv)
    }
}

/// Normalize a Mux
fn make_mux(s: Signal, a: Signal, b: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    if s.is_inverted() {
        make_mux(!s, b, a, inv)
    } else if b.is_inverted() {
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
        Node(Ternary([s, a, b], TernaryType::Mux), inv)
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
    } else if i0.is_inverted() {
        // Won't cause an infinite loop because the order will not change
        // We already removed cases where signals differ by their sign
        make_maj(!i0, !i1, !i2, !inv)
    } else if i0 == Signal::zero() {
        make_and(i1, i2, inv)
    } else {
        Node(Ternary([i0, i1, i2], TernaryType::Maj), inv)
    }
}

/// Normalize a Dff
fn make_dff(d: Signal, en: Signal, res: Signal, inv: bool) -> Normalization {
    use Gate::*;
    use Normalization::*;
    if d == Signal::zero() || en == Signal::zero() || res == Signal::one() {
        Copy(Signal::zero() ^ inv)
    } else {
        Node(Dff([d, en, res]), inv)
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
        pol ^= s.is_inverted();
    }
    for s in &mut vs {
        *s = s.without_inversion();
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
                Binary([a, b], BinaryType::And) => make_and(*a, *b, *inv),
                Binary([a, b], BinaryType::Xor) => make_xor(*a, *b, *inv),
                Ternary([a, b, c], TernaryType::And) => make_and3(*a, *b, *c, *inv),
                Ternary([a, b, c], TernaryType::Xor) => make_xor3(*a, *b, *c, *inv),
                Ternary([s, a, b], TernaryType::Mux) => make_mux(*s, *a, *b, *inv),
                Ternary([a, b, c], TernaryType::Maj) => make_maj(*a, *b, *c, *inv),
                Dff([d, en, res]) => make_dff(*d, *en, *res, *inv),
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
                Lut(_) => self.clone(),
            },
        }
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Gate::*;
        match self {
            Binary([a, b], BinaryType::And) => {
                write!(f, "{a} & {b}")
            }
            Binary([a, b], BinaryType::Xor) => {
                write!(f, "{a} ^ {b}")
            }
            Ternary([a, b, c], TernaryType::And) => {
                write!(f, "{a} & {b} & {c}")
            }
            Ternary([a, b, c], TernaryType::Xor) => {
                write!(f, "{a} ^ {b} ^ {c}")
            }
            Ternary([s, a, b], TernaryType::Mux) => {
                write!(f, "{s} ? {a} : {b}")
            }
            Ternary([a, b, c], TernaryType::Maj) => {
                write!(f, "Maj({a}, {b}, {c})")
            }
            Dff([d, en, res]) => {
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
                    NaryType::And | NaryType::Or | NaryType::Xor => false,
                };
                let st = v
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(sep);
                if inv {
                    write!(f, "!({})", st)
                } else {
                    write!(f, "{}", st)
                }
            }
            Buf(s) => {
                write!(f, "{}", s)
            }
            Lut(lut) => {
                let st = lut
                    .inputs
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{}({})", lut.lut, st)
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
    !a.is_inverted() && !b.is_inverted()
}

fn no_inv_3(a: Signal, b: Signal, c: Signal) -> bool {
    !a.is_inverted() && !b.is_inverted() && !c.is_inverted()
}

fn no_inv_n(v: &[Signal]) -> bool {
    v.iter().all(|s| !s.is_inverted())
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
                check_canonization(Gate::and(*i0, *i1));
                check_canonization(Gate::xor(*i0, *i1));
                for i2 in vars.iter() {
                    check_canonization(Gate::mux(*i0, *i1, *i2));
                    check_canonization(Gate::maj(*i0, *i1, *i2));
                    check_canonization(Gate::and3(*i0, *i1, *i2));
                    check_canonization(Gate::xor3(*i0, *i1, *i2));
                    check_canonization(Gate::dff(*i0, *i1, *i2));
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
        assert!(Gate::and(i0, i1).is_canonical());
        assert!(Gate::and(i0, !i1).is_canonical());
        assert!(Gate::and(!i0, i1).is_canonical());
        assert!(Gate::and(!i0, !i1).is_canonical());

        // Wrong ordering
        assert!(!Gate::and(i1, i0).is_canonical());
        assert!(!Gate::and(i1, !i0).is_canonical());
        assert!(!Gate::and(!i1, i0).is_canonical());
        assert!(!Gate::and(!i1, !i0).is_canonical());

        // Constant
        assert!(!Gate::and(l0, i1).is_canonical());
        assert!(!Gate::and(l1, i1).is_canonical());

        // Repeatition
        assert!(!Gate::and(i0, i0).is_canonical());
        assert!(!Gate::and(i0, !i0).is_canonical());
    }

    #[test]
    fn test_xor_is_canonical() {
        let l0 = Signal::zero();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);

        // Everything OK
        assert!(Gate::xor(i0, i1).is_canonical());

        // Wrong ordering
        assert!(!Gate::xor(i1, i0).is_canonical());

        // Bad polarity
        assert!(!Gate::xor(i0, !i1).is_canonical());
        assert!(!Gate::xor(!i0, i1).is_canonical());

        // Constant
        assert!(!Gate::xor(l0, i1).is_canonical());

        // Repeatition
        assert!(!Gate::xor(i0, i0).is_canonical());
    }

    #[test]
    fn test_maj_is_canonical() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Everything OK
        assert!(Gate::maj(i0, i1, i2).is_canonical());
        assert!(Gate::maj(i0, !i1, i2).is_canonical());
        assert!(Gate::maj(i0, !i1, i2).is_canonical());
        assert!(Gate::maj(i0, !i1, !i2).is_canonical());

        // Wrong ordering
        assert!(!Gate::maj(i0, i2, i1).is_canonical());
        assert!(!Gate::maj(i1, i0, i2).is_canonical());

        // Constant
        assert!(!Gate::maj(l0, i1, i2).is_canonical());
        assert!(!Gate::maj(l1, i1, i2).is_canonical());

        // Wrong polarity
        assert!(!Gate::maj(!i0, i1, i2).is_canonical());
        assert!(!Gate::maj(!i0, !i1, i2).is_canonical());
        assert!(!Gate::maj(!i0, !i1, i2).is_canonical());
        assert!(!Gate::maj(!i0, !i1, !i2).is_canonical());

        // Repeatition
        assert!(!Gate::maj(i0, i0, i2).is_canonical());
        assert!(!Gate::maj(i0, !i0, i2).is_canonical());
        assert!(!Gate::maj(i0, i2, i2).is_canonical());
        assert!(!Gate::maj(i0, i2, !i2).is_canonical());
    }

    #[test]
    fn test_mux_is_canonical() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        let i0 = Signal::from_var(0);
        let i1 = Signal::from_var(1);
        let i2 = Signal::from_var(2);

        // Everything OK
        assert!(Gate::mux(i2, i1, i0).is_canonical());
        assert!(Gate::mux(i2, !i1, i0).is_canonical());

        // Bad polarity
        assert!(!Gate::mux(i2, i1, !i0).is_canonical());
        assert!(!Gate::mux(i2, !i1, !i0).is_canonical());
        assert!(!Gate::mux(!i2, i1, i0).is_canonical());
        assert!(!Gate::mux(!i2, !i1, i0).is_canonical());

        // Constant anywhere
        assert!(!Gate::mux(l0, i1, i0).is_canonical());
        assert!(!Gate::mux(i2, l0, i0).is_canonical());
        assert!(!Gate::mux(i2, i1, l0).is_canonical());
        assert!(!Gate::mux(i2, i1, !l0).is_canonical());
        assert!(!Gate::mux(l1, i1, i0).is_canonical());
        assert!(!Gate::mux(i2, l1, i0).is_canonical());
        assert!(!Gate::mux(i2, i1, l1).is_canonical());
        assert!(!Gate::mux(i2, i1, !l1).is_canonical());

        // Repeatition anywhere
        assert!(!Gate::mux(i2, i2, i0).is_canonical());
        assert!(!Gate::mux(i0, i2, i2).is_canonical());
        assert!(!Gate::mux(i2, i0, i2).is_canonical());
        assert!(!Gate::mux(i2, !i2, i0).is_canonical());
        assert!(!Gate::mux(i0, i2, !i2).is_canonical());
        assert!(!Gate::mux(i2, i0, !i2).is_canonical());
        assert!(!Gate::mux(!i2, i2, i0).is_canonical());
        assert!(!Gate::mux(i0, !i2, i2).is_canonical());
        assert!(!Gate::mux(!i2, i0, i2).is_canonical());
    }

    /// Check that the size used for Gate does not increase
    ///
    /// This is currently too high due to the NAry variant, where the Box uses 16 bytes.
    /// This could be made lower with another level of indirection, or with an ad-hoc type
    /// to replace Box.
    #[test]
    fn test_representation_size() {
        assert!(std::mem::size_of::<Gate>() <= 6 * std::mem::size_of::<Signal>());
    }
}
