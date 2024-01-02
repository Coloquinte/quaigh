//! Compute an approximation of the area or of the complexity of a network
//!
//! ```
//! # use quaigh::Network;
//! # let aig = Network::new();
//! use quaigh::network::area::AreaParameters;
//!
//! // To estimate area for VLSI designs
//! println!("VLSI cost: {}", AreaParameters::vlsi().area(&aig));
//!
//! // To estimate area for FPGA designs
//! println!("FPGA cost: {}", AreaParameters::fpga().area(&aig));
//!
//! // To estimate complexity for SAT solving and proofs
//! println!("SAT cost: {}", AreaParameters::sat().area(&aig));
//! ```

use std::fmt;

use crate::network::gates::{BinaryType, TernaryType};
use crate::{Gate, NaryType, Network};

/// Area estimation parameters for optimization
///
/// Most gates have an area cost. N-ary gates are extrapolated and buffers are ignored.
/// This is obviously very inaccurate, and is meant to be used as an objective during logic optimization.
#[derive(Clone, Copy, Debug)]
pub struct AreaParameters {
    /// Cost of And2
    pub and: usize,
    /// Cost of And3
    pub and3: usize,
    /// Cost of Xor2
    pub xor: usize,
    /// Cost of Xor3
    pub xor3: usize,
    /// Cost of Mux
    pub mux: usize,
    /// Cost of Maj
    pub maj: usize,
    /// Cost of Dff
    pub dff: usize,
}

impl AreaParameters {
    /// Good default parameters for VLSI design
    ///
    /// In VLSI, And gates are cheap and easy to merge together, while Xor is more expensive.
    /// We use roughly the area the cells would use in a standard cell library.
    pub fn vlsi() -> AreaParameters {
        AreaParameters {
            and: 4,
            and3: 6,
            xor: 8,
            xor3: 16,
            mux: 9,
            maj: 6,
            dff: 24,
        }
    }

    /// Good default parameters for FPGA design
    ///
    /// FPGAs represent all logic functions with LUTs, whose cost only depends on the number of inputs.
    pub fn fpga() -> AreaParameters {
        AreaParameters {
            and: 2,
            and3: 3,
            xor: 2,
            xor3: 3,
            mux: 3,
            maj: 3,
            dff: 4,
        }
    }

    /// Good default parameters for SAT solving
    ///
    /// We use the number of literals in the formula as a proxy for solving complexity.
    pub fn sat() -> AreaParameters {
        AreaParameters {
            and: 7,
            and3: 10,
            xor: 12,
            xor3: 24,
            mux: 13,
            maj: 18,
            dff: 20,
        }
    }

    /// Extrapolate the cost of the n-ary and
    fn andn(&self, n: usize) -> usize {
        if n < 2 {
            0
        } else {
            self.and + (n - 2) * (self.and3 - self.and)
        }
    }

    /// Extrapolate the cost of the n-ary xor
    fn xorn(&self, n: usize) -> usize {
        if n < 2 {
            0
        } else {
            self.xor + (n - 2) * (self.xor3 - self.xor)
        }
    }

    /// Compute the area of a gate
    pub fn gate_area(&self, g: &Gate) -> usize {
        use Gate::*;
        match g {
            Binary(_, BinaryType::And) => self.and,
            Ternary(_, TernaryType::And) => self.and3,
            Binary(_, BinaryType::Xor) => self.xor,
            Ternary(_, TernaryType::Xor) => self.xor3,
            Nary(v, tp) => match tp {
                NaryType::And | NaryType::Or | NaryType::Nand | NaryType::Nor => self.andn(v.len()),
                NaryType::Xor | NaryType::Xnor => self.xorn(v.len()),
            },
            Dff(_) => self.dff,
            Ternary(_, TernaryType::Mux) => self.mux,
            Ternary(_, TernaryType::Maj) => self.maj,
            Buf(_) => 0,
        }
    }

    /// Compute the area of a network
    pub fn area(&self, a: &Network) -> usize {
        let mut ret = 0;
        for i in 0..a.nb_nodes() {
            ret += self.gate_area(a.gate(i));
        }
        ret
    }

    /// Perform a consistency check to verify that the parameters are consistent
    pub fn check(&self) {
        // Everything positive (except maybe Dff)
        assert!(self.and > 0);
        assert!(self.xor > 0);
        assert!(self.and3 > 0);
        assert!(self.xor3 > 0);
        assert!(self.mux > 0);
        assert!(self.maj > 0);

        // Strict cost to having more inputs
        assert!(self.and3 > self.and);
        assert!(self.xor3 > self.xor);
        assert!(self.maj > self.and);

        // Do not force usage of small arities
        assert!(self.and3 <= 2 * self.and);
        assert!(self.xor3 <= 2 * self.xor);

        // Do not force replacement of And/Xor by Mux
        assert!(self.xor < self.mux);
        assert!(self.and < self.mux);
    }
}

impl fmt::Display for AreaParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Area costs:")?;
        writeln!(f, "  And2: {}", self.and)?;
        writeln!(f, "  And3: {}", self.and3)?;
        writeln!(f, "  Xor2: {}", self.xor)?;
        writeln!(f, "  Xor3: {}", self.xor3)?;
        writeln!(f, "  Mux: {}", self.mux)?;
        writeln!(f, "  Maj: {}", self.maj)?;
        writeln!(f, "  Dff: {}", self.dff)?;
        fmt::Result::Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AreaParameters;

    #[test]
    fn test_consistent() {
        AreaParameters::vlsi().check();
        AreaParameters::fpga().check();
        AreaParameters::sat().check();
    }
}
