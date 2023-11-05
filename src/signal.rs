use std::{
    fmt,
    ops::{BitXor, BitXorAssign, Not},
};

/// Representation of a literal (a boolean variable or its complement). May be 0, 1, x or !x.
/// Design inputs and constants get a special representation.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug, Default)]
pub struct Signal {
    a: u32,
}

impl Signal {
    /// Constant zero literal
    pub fn zero() -> Signal {
        Signal { a: 0 }
    }

    /// Constant one literal
    pub fn one() -> Signal {
        Signal { a: 1 }
    }

    /// Create a literal from a variable index
    pub fn from_var(v: u32) -> Signal {
        Self::from_ind(v + 1)
    }

    /// Create a literal from a design input index
    pub fn from_input(v: u32) -> Signal {
        Self::from_ind(!v)
    }

    /// Create a literal from an index (including zero literal at index 0)
    pub(crate) fn from_ind(v: u32) -> Signal {
        Signal { a: v << 1 }
    }

    /// Obtain the variable index associated with the literal
    pub fn var(&self) -> u32 {
        assert!(self.is_var());
        self.ind() - 1u32
    }

    /// Obtain the design input index associated with the literal
    pub fn input_ind(&self) -> u32 {
        assert!(self.is_input());
        !self.ind() & !0x8000_0000
    }

    /// Obtain the internal index associated with the literal: 0 for a constant, otherwise var() + 1
    pub fn ind(&self) -> u32 {
        self.a >> 1
    }

    /// Obtain the polarity of the literal (True for a complemented variable)
    pub fn pol(&self) -> bool {
        self.a & 1 != 0
    }

    /// Returns true if the literal represents a constant
    pub fn is_constant(&self) -> bool {
        self.ind() == 0
    }

    /// Returns true if the literal represents a design input
    pub fn is_input(&self) -> bool {
        self.a & 0x7000000 != 0
    }

    /// Returns true if the literal represents an internal variable
    pub fn is_var(&self) -> bool {
        !self.is_input() && !self.is_constant()
    }

    /// Clear the polarity
    pub(crate) fn without_pol(&self) -> Signal {
        Signal { a: self.a & !1u32 }
    }

    /// Convert the polarity to a word for bitwise operations
    pub(crate) fn pol_to_word(&self) -> u64 {
        let pol = self.a & 1;
        !(pol as u64) + 1
    }

    /// Apply a variable remapping to the signal
    pub(crate) fn remap(&self, t: &[Signal]) -> Signal {
        if !self.is_var() {
            *self
        } else {
            t[self.var() as usize] ^ self.pol()
        }
    }
}

impl From<bool> for Signal {
    fn from(b: bool) -> Signal {
        if b {
            Signal::one()
        } else {
            Signal::zero()
        }
    }
}

impl Not for Signal {
    type Output = Signal;
    fn not(self) -> Signal {
        Signal { a: self.a ^ 1u32 }
    }
}

impl Not for &'_ Signal {
    type Output = Signal;
    fn not(self) -> Signal {
        Signal { a: self.a ^ 1u32 }
    }
}

impl BitXorAssign<bool> for Signal {
    fn bitxor_assign(&mut self, rhs: bool) {
        self.a ^= rhs as u32;
    }
}

impl BitXor<bool> for Signal {
    type Output = Signal;
    fn bitxor(self, rhs: bool) -> Self::Output {
        let mut l = self;
        l ^= rhs;
        l
    }
}

impl BitXor<bool> for &'_ Signal {
    type Output = Signal;
    fn bitxor(self, rhs: bool) -> Self::Output {
        let mut l = *self;
        l ^= rhs;
        l
    }
}

impl BitXor<&bool> for Signal {
    type Output = Signal;
    fn bitxor(self, rhs: &bool) -> Self::Output {
        self ^ *rhs
    }
}

impl BitXor<&bool> for &'_ Signal {
    type Output = Signal;
    fn bitxor(self, rhs: &bool) -> Self::Output {
        self ^ *rhs
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_constant() {
            let a = self.a & 1;
            write!(f, "{a}")
        } else {
            if self.pol() {
                write!(f, "!")?;
            }
            if self.is_input() {
                // Representation of inputs
                let v = self.input_ind();
                write!(f, "i{v}")
            } else {
                let v = self.var();
                write!(f, "x{v}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal() {
        let l0 = Signal::zero();
        let l1 = Signal::one();
        assert_eq!(l0, !l1);
        assert_eq!(l1, !l0);
        assert!(!l0.pol());
        assert!(l1.pol());
        assert_eq!(format!("{l0}"), "0");
        assert_eq!(format!("{l1}"), "1");
        for v in 0u32..10u32 {
            let l = Signal::from_var(v);
            assert_eq!(l.var(), v);
            assert_eq!((!l).var(), v);
            assert!(!l.pol());
            assert!((!l).pol());
            assert_eq!(l ^ false, l);
            assert_eq!(l ^ true, !l);
            assert_eq!(format!("{l}"), format!("x{v}"));
        }
        for v in 0u32..10u32 {
            let l = Signal::from_input(v);
            assert_eq!(l.input_ind(), v);
            assert_eq!((!l).input_ind(), v);
            assert!(!l.pol());
            assert!((!l).pol());
            assert_eq!(l ^ false, l);
            assert_eq!(l ^ true, !l);
            assert_eq!(format!("{l}"), format!("i{v}"));
        }
    }

    #[test]
    fn test_comparison() {
        // Boolean conversion
        assert_eq!(Signal::from(false), Signal::zero());
        assert_eq!(Signal::from(true), Signal::one());
        assert_ne!(Signal::from(false), Signal::one());
        assert_ne!(Signal::from(true), Signal::zero());

        // Design variable
        assert_ne!(Signal::from_var(0), Signal::one());
        assert_ne!(Signal::from_var(0), Signal::zero());
        assert_ne!(Signal::from_var(0), Signal::from_var(1));
        assert_ne!(Signal::from_var(0), Signal::from_var(1));

        // Design input
        assert_ne!(Signal::from_input(0), Signal::from_var(0));
        assert_ne!(Signal::from_input(0), Signal::from_var(0));
        assert_ne!(Signal::from_input(0), Signal::one());
        assert_ne!(Signal::from_input(0), Signal::zero());

        // Xor
        assert_eq!(Signal::zero() ^ false, Signal::zero());
        assert_eq!(Signal::zero() ^ true, Signal::one());
        assert_eq!(Signal::one() ^ false, Signal::one());
        assert_eq!(Signal::one() ^ true, Signal::zero());
    }
}
