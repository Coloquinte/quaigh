//! Compute gate statistics on Aigs
//!
//! ```
//! # use quaigh::Aig;
//! # let aig = Aig::new();
//! use quaigh::stats::stats;
//! let stats = stats(&aig);
//!
//! // Check that there is no Xor2 gate
//! assert_eq!(stats.nb_xor, 0);
//!
//! // Show the statistics
//! println!("{}", stats);
//! ```

use std::fmt;

use crate::{Aig, Gate, NaryType};

/// Number of inputs, outputs and gates in an Aig
#[derive(Clone, Copy, Debug)]
pub struct NetworkStats {
    /// Number of inputs
    pub nb_inputs: usize,
    /// Number of outputs
    pub nb_outputs: usize,
    /// Number of And2
    pub nb_and: usize,
    /// Number of And3
    pub nb_and3: usize,
    /// Number of Andn
    pub nb_andn: usize,
    /// Number of Xor2
    pub nb_xor: usize,
    /// Number of Xor3
    pub nb_xor3: usize,
    /// Number of Xorn
    pub nb_xorn: usize,
    /// Number of Mux
    pub nb_mux: usize,
    /// Number of Maj
    pub nb_maj: usize,
    /// Number of Buf
    pub nb_buf: usize,
    /// Number of Dff
    pub nb_dff: usize,
}

impl NetworkStats {
    /// Total number of gates, including Dff
    pub fn nb_gates(&self) -> usize {
        self.nb_and
            + self.nb_and3
            + self.nb_andn
            + self.nb_xor
            + self.nb_xor3
            + self.nb_xorn
            + self.nb_mux
            + self.nb_maj
            + self.nb_buf
            + self.nb_dff
    }
}

impl fmt::Display for NetworkStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Stats:")?;
        writeln!(f, "  Inputs: {}", self.nb_inputs)?;
        writeln!(f, "  Outputs: {}", self.nb_outputs)?;
        writeln!(f, "  Gates: {}", self.nb_gates())?;
        writeln!(f, "  Dff: {}", self.nb_dff)?;
        if self.nb_and != 0 {
            writeln!(f, "  And2: {}", self.nb_and)?;
        }
        if self.nb_and3 != 0 {
            writeln!(f, "  And3: {}", self.nb_and3)?;
        }
        if self.nb_andn != 0 {
            writeln!(f, "  Andn: {}", self.nb_andn)?;
        }
        if self.nb_xor != 0 {
            writeln!(f, "  Xor2: {}", self.nb_xor)?;
        }
        if self.nb_xor3 != 0 {
            writeln!(f, "  Xor3: {}", self.nb_xor3)?;
        }
        if self.nb_xorn != 0 {
            writeln!(f, "  Xorn: {}", self.nb_xorn)?;
        }
        if self.nb_mux != 0 {
            writeln!(f, "  Mux: {}", self.nb_mux)?;
        }
        if self.nb_maj != 0 {
            writeln!(f, "  Maj: {}", self.nb_maj)?;
        }
        if self.nb_buf != 0 {
            writeln!(f, "  Buf: {}", self.nb_buf)?;
        }
        fmt::Result::Ok(())
    }
}

/// Compute the statistics of the Aig
pub fn stats(a: &Aig) -> NetworkStats {
    use Gate::*;
    let mut ret = NetworkStats {
        nb_inputs: a.nb_inputs(),
        nb_outputs: a.nb_outputs(),
        nb_and: 0,
        nb_and3: 0,
        nb_andn: 0,
        nb_xor: 0,
        nb_xor3: 0,
        nb_xorn: 0,
        nb_maj: 0,
        nb_mux: 0,
        nb_buf: 0,
        nb_dff: 0,
    };
    for i in 0..a.nb_nodes() {
        match a.gate(i) {
            And(_, _) => ret.nb_and += 1,
            And3(_, _, _) => ret.nb_and3 += 1,
            Xor(_, _) => ret.nb_xor += 1,
            Xor3(_, _, _) => ret.nb_xor3 += 1,
            Mux(_, _, _) => ret.nb_mux += 1,
            Maj(_, _, _) => ret.nb_maj += 1,
            Buf(_) => ret.nb_buf += 1,
            Dff(_, _, _) => ret.nb_dff += 1,
            Nary(_, tp) => match tp {
                NaryType::And | NaryType::Or | NaryType::Nand | NaryType::Nor => ret.nb_andn += 1,
                NaryType::Xor | NaryType::Xnor => ret.nb_xorn += 1,
            },
        }
    }

    ret
}
