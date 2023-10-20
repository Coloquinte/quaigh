use std::{
    fmt,
    ops::{BitXor, BitXorAssign, Not},
};

/// Representation of a literal (a boolean variable or its complement). May be 0, 1, x or !x.
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

    /// Create a literal from a boolean variable index
    pub fn from_var(v: u32) -> Signal {
        Signal { a: (v + 1) << 2 }
    }

    /// Create a literal from an index
    pub(crate) fn from_ind(v: u32) -> Signal {
        Signal { a: v << 2 }
    }

    /// Obtain the variable number associated with the literal
    pub fn var(&self) -> u32 {
        let v = self.a >> 2;
        assert!(v > 0);
        v - 1u32
    }

    /// Obtain the internal index associated with the literal: 0 for a constant, otherwise var() + 1
    pub fn ind(&self) -> u32 {
        self.a >> 2
    }

    /// Obtain the polarity of the literal (True for a complemented variable)
    pub fn pol(&self) -> bool {
        self.a & 1 != 0
    }

    /// Returns true if the literal represents a constant
    pub fn is_constant(&self) -> bool {
        self.ind() == 0
    }

    /// Obtain the additional flag in the literal
    /// Should always be 0 outside of internal representations
    pub(crate) fn flag(&self) -> bool {
        self.a & 2 != 0
    }

    /// Clear the flag
    pub(crate) fn without_flag(&self) -> Signal {
        Signal { a: self.a & !2u32 }
    }

    /// Set the flag
    pub(crate) fn with_flag(&self) -> Signal {
        Signal { a: self.a | 2u32 }
    }

    /// Clear the polarity
    pub(crate) fn without_pol(&self) -> Signal {
        Signal { a: self.a & !1u32 }
    }

    /// Set the polarity
    pub(crate) fn with_pol(&self) -> Signal {
        Signal { a: self.a | 1u32 }
    }

    /// Convert the polarity to a word for bitwise operations
    pub(crate) fn pol_to_word(&self) -> u64 {
        let pol = self.a & 1;
        !(pol as u64) + 1
    }

    /// Convert the flag to a word for bitwise operations
    pub(crate) fn flag_to_word(&self) -> u64 {
        let flag = (self.a >> 1) & 1;
        !(flag as u64) + 1
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

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_constant() {
            let a = self.a & 1;
            write!(f, "{a}")
        } else {
            if self.pol() {
                write!(f, "!")?;
            }
            let v = self.var();
            write!(f, "v{v}")
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
            assert_eq!(format!("{l}"), format!("v{v}"));
        }
    }
}
