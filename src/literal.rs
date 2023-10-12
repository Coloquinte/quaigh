use std::{
    fmt,
    ops::{Add, BitAnd, BitOr, BitXor, Not},
};

pub(crate) trait Num:
    Sized
    + Not<Output = Self>
    + BitOr<Output = Self>
    + BitAnd<Output = Self>
    + BitXor<Output = Self>
    + Add<Output = Self>
    + From<u32>
    + Copy
{
}

/// Representation of a literal
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug, Default)]
pub struct Lit {
    a: u32,
}

impl Lit {
    // Constant zero literal
    pub fn zero() -> Lit {
        Lit { a: 0 }
    }

    // Constant one literal
    pub fn one() -> Lit {
        Lit { a: 1 }
    }

    pub fn from_var(v: u32) -> Lit {
        Lit { a: (v + 1) << 2 }
    }

    // Returns true if the literal represents a constant
    pub fn is_constant(&self) -> bool {
        self.a >> 2 == 0
    }

    /// Obtain the variable ID associated with the literal
    pub fn var(&self) -> u32 {
        self.variable()
    }

    /// Obtain the variable ID associated with the literal
    /// 
    /// Synonym of [`var`]
    pub fn variable(&self) -> u32 {
        let v = self.a >> 2;
        assert!(v > 0);
        v - 1u32
    }

    /// Obtain the polarity of the literal (True for an inversion)
    pub fn pol(&self) -> bool {
        self.polarity()
    }

    /// Obtain the polarity of the literal (True for an inversion)
    /// 
    /// Synonym of [`pol`]
    pub fn polarity(&self) -> bool {
        self.a & 1 != 0
    }

    /// Obtain the additional flag in the literal
    /// Should always be 0 outside of internal representations
    pub(crate) fn flag(&self) -> bool {
        self.a & 2 != 0
    }

    /// Clear the flag
    pub(crate) fn without_flag(&self) -> Lit {
        Lit { a: self.a & !2u32 }
    }

    /// Set the flag
    pub(crate) fn with_flag(&self) -> Lit {
        Lit { a: self.a | 2u32 }
    }

    /// Clear the polarity
    pub(crate) fn without_pol(&self) -> Lit {
        Lit { a: self.a & !1u32 }
    }

    /// Set the polarity
    pub(crate) fn with_pol(&self) -> Lit {
        Lit { a: self.a | 1u32 }
    }

    /// Convert the polarity to a word for bitwise operations
    pub(crate) fn pol_to_word<T: Num>(&self) -> T {
        let pol = self.a & 1;
        !T::from(pol) + T::from(1)
    }

    /// Convert the flag to a word for bitwise operations
    pub(crate) fn flag_to_word<T: Num>(&self) -> T {
        let flag = (self.a >> 1) & 1;
        !T::from(flag) + T::from(1)
    }

    /// Obtain ID associated with the literal
    /// 
    /// 0 for a constant, otherwise var() + 1
    pub fn ind(&self) -> u32 {
        self.a >> 2
    }
}

impl Not for Lit {
    type Output = Lit;
    fn not(self) -> Lit {
        Lit { a: self.a ^ 1u32 }
    }
}

impl Not for &'_ Lit {
    type Output = Lit;
    fn not(self) -> Lit {
        Lit { a: self.a ^ 1u32 }
    }
}

impl fmt::Display for Lit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_constant() {
            let a = self.a;
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
        let l0 = Lit::zero();
        let l1 = Lit::one();
        assert_eq!(l0, !l1);
        assert_eq!(l1, !l0);
        assert_eq!(l0.pol(), false);
        assert_eq!(l1.pol(), true);
        assert_eq!(format!("{l0}"), "0");
        assert_eq!(format!("{l1}"), "1");
        for v in 0u32..10u32 {
            let l = Lit::from_var(v);
            assert_eq!(l.var(), v);
            assert_eq!((!l).var(), v);
            assert_eq!(l.pol(), false);
            assert_eq!((!l).pol(), true);
            assert_eq!(format!("{l}"), format!("v{v}"));
        }
    }
}
