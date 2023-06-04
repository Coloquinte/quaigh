use std::{
    cmp, fmt,
};

use crate::literal::Lit;
use crate::literal::Num;

/// Representation of an AIG node
struct AigNode {
    a: Lit,
    b: Lit,
    c: Lit,
}

/// Represent the result of the normalization of a node.
/// Either a literal if it can be simplified, or a canonical representation of an AIG node
enum NormalizationResult {
    /// Directly a literal
    Literal(Lit),
    PosNode(AigNode),
    NegNode(AigNode),
}

/// Represent the different basic gates that can be represented
enum BasicGate {
    And(Lit, Lit),
    Xor(Lit, Lit),
    Mux(Lit, Lit, Lit),
    Maj(Lit, Lit, Lit),
}

impl AigNode {
    fn simulate<T: Num>(&self, a_val: T, b_val: T, c_val: T) -> T {
        // Convert boolean flags to full-width words
        let toggle_a: T = self.a.pol_to_word();
        let toggle_b: T = self.b.pol_to_word();
        let toggle_c: T = self.c.pol_to_word();
        let sel_mux: T = self.a.flag_to_word();
        let av: T = a_val ^ toggle_a;
        let bv: T = b_val ^ toggle_b;
        let cv: T = c_val ^ toggle_c;
        let mux: T = (av & bv) | (!av & cv);
        let maj: T = (av & bv) | (av & cv) | (bv & cv);
        (!sel_mux & maj) | (sel_mux & mux)
    }

    fn is_canonical(&self) -> bool {
        let AigNode { a, b, c } = self;
        if a.flag() {
            // Mux
            if a.is_constant() || b.is_constant() || c.is_constant() {
                // No constant at all allowed
                return false;
            }
            if a.polarity() {
                // No inversion on a
                return false;
            }
            if a.variable() == b.variable() || a.variable() == c.variable() {
                // No sharing between selector and other variables
                return false;
            }
            if b.variable() == c.variable() {
                // Xor
                if c.polarity() {
                    // Pick a polarity
                    return false;
                }
                if a <= b {
                    // Use largest as selector
                    return false;
                }
            }
            return true;
        } else {
            // Maj
            if a <= b || b <= c {
                // Force strict ordering on the inputs, a > b > c
                return false;
            }
            if c == Lit::zero() {
                // Only constant one on the last input, representing an And
                return false;
            }
            return true;
        }
    }

    // Normalize an and gate
    fn make_canonical_and(a: Lit, b: Lit) -> NormalizationResult {
        assert!(!a.flag() && !b.flag());
        let mn = cmp::min(a, b);
        let mx = cmp::max(a, b);
        if mn == Lit::zero() {
            NormalizationResult::Literal(Lit::zero())
        }
        if mn == Lit::one() {
            NormalizationResult::Literal(mx)
        }
        if mn == mx {
            NormalizationResult::Literal(mn)
        }
        if mn == !mx {
            NormalizationResult::Literal(Lit::zero())
        }
        NormalizationResult::PosNode(AigNode {
            a: mx,
            b: mn,
            c: Lit::one(),
        })
    }

    // Normalize a mux gate
    fn make_canonical_mux(s: Lit, a: Lit, b: Lit) -> NormalizationResult {
        if s == Lit::zero() {
            return NormalizationResult::Literal(b);
        }
        if s == Lit::one() {
            return NormalizationResult::Literal(a);
        }
    }

    fn make_canonical_maj(i1: Lit, i2: Lit, i3: Lit) -> NormalizationResult {
        // TODO: sort
    }

    fn make_canonical(&self) -> NormalizationResult {
        // Normalize a majority
        // Sort the inputs
        // Two inputs constant
        // One input constant
    }
}

impl fmt::Display for AigNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (a, b, c) = (self.a, self.b, self.c);
        if a.flag() {
            write!(f, "Mux()")?;
        } else {
            write!(f, "Maj()")?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let l1 = Lit::zero();
    }
}
