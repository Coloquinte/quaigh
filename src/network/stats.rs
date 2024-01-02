//! Compute gate statistics
//!
//! ```
//! # use quaigh::Network;
//! # let aig = Network::new();
//! use quaigh::network::stats::stats;
//! let stats = stats(&aig);
//!
//! // Check that there is no Xor2 gate
//! assert_eq!(stats.nb_xor, 0);
//!
//! // Show the statistics
//! println!("{}", stats);
//! ```

use std::fmt;

use crate::network::gates::{BinaryType, TernaryType};
use crate::{Gate, NaryType, Network};

/// Number of inputs, outputs and gates in a network
#[derive(Clone, Debug)]
pub struct NetworkStats {
    /// Number of inputs
    pub nb_inputs: usize,
    /// Number of outputs
    pub nb_outputs: usize,
    /// Number of And and similar gates
    pub nb_and: usize,
    /// Arity of And gates
    pub and_arity: Vec<usize>,
    /// Number of Xor and similar gates
    pub nb_xor: usize,
    /// Arity of Xor gates
    pub xor_arity: Vec<usize>,
    /// Number of Mux
    pub nb_mux: usize,
    /// Number of Maj
    pub nb_maj: usize,
    /// Number of positive Buf
    pub nb_buf: usize,
    /// Number of Not (negative Buf)
    pub nb_not: usize,
    /// Number of Dff
    pub nb_dff: usize,
    /// Number of Dff with enable
    pub nb_dffe: usize,
    /// Number of Dff with reset
    pub nb_dffr: usize,
}

impl NetworkStats {
    /// Total number of gates, including Dff
    pub fn nb_gates(&self) -> usize {
        self.nb_and + self.nb_xor + self.nb_mux + self.nb_maj + self.nb_buf + self.nb_dff
    }

    /// Record a new and
    fn add_and(&mut self, sz: usize) {
        self.nb_and += 1;
        while self.and_arity.len() <= sz {
            self.and_arity.push(0);
        }
        self.and_arity[sz] += 1;
    }

    /// Record a new xor
    fn add_xor(&mut self, sz: usize) {
        self.nb_xor += 1;
        while self.xor_arity.len() <= sz {
            self.xor_arity.push(0);
        }
        self.xor_arity[sz] += 1;
    }
}

impl fmt::Display for NetworkStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Stats:")?;
        writeln!(f, "  Inputs: {}", self.nb_inputs)?;
        writeln!(f, "  Outputs: {}", self.nb_outputs)?;
        writeln!(f, "  Gates: {}", self.nb_gates())?;
        if self.nb_dff != 0 {
            writeln!(f, "  Dff: {}", self.nb_dff)?;
            if self.nb_dffe != 0 {
                writeln!(f, "      enable: {}", self.nb_dff)?;
            }
            if self.nb_dffr != 0 {
                writeln!(f, "      reset: {}", self.nb_dff)?;
            }
        }
        if self.nb_and != 0 {
            writeln!(f, "  And: {}", self.nb_and)?;
            for (i, nb) in self.and_arity.iter().enumerate() {
                if *nb != 0 {
                    writeln!(f, "      {}: {}", i, nb)?;
                }
            }
        }
        if self.nb_xor != 0 {
            writeln!(f, "  Xor: {}", self.nb_xor)?;
            for (i, nb) in self.xor_arity.iter().enumerate() {
                if *nb != 0 {
                    writeln!(f, "      {}: {}", i, nb)?;
                }
            }
        }
        if self.nb_mux != 0 {
            writeln!(f, "  Mux: {}", self.nb_mux)?;
        }
        if self.nb_maj != 0 {
            writeln!(f, "  Maj: {}", self.nb_maj)?;
        }
        if self.nb_not != 0 {
            writeln!(f, "  Not: {}", self.nb_not)?;
        }
        if self.nb_buf != 0 {
            writeln!(f, "  Buf: {}", self.nb_buf)?;
        }
        fmt::Result::Ok(())
    }
}

/// Compute the statistics of the network
pub fn stats(a: &Network) -> NetworkStats {
    use Gate::*;
    let mut ret = NetworkStats {
        nb_inputs: a.nb_inputs(),
        nb_outputs: a.nb_outputs(),
        nb_and: 0,
        and_arity: Vec::new(),
        nb_xor: 0,
        xor_arity: Vec::new(),
        nb_maj: 0,
        nb_mux: 0,
        nb_buf: 0,
        nb_not: 0,
        nb_dff: 0,
        nb_dffe: 0,
        nb_dffr: 0,
    };
    for i in 0..a.nb_nodes() {
        match a.gate(i) {
            Binary(_, BinaryType::And) => ret.add_and(2),
            Ternary(_, TernaryType::And) => ret.add_and(3),
            Binary(_, BinaryType::Xor) => ret.add_xor(2),
            Ternary(_, TernaryType::Xor) => ret.add_xor(3),
            Ternary(_, TernaryType::Mux) => ret.nb_mux += 1,
            Ternary(_, TernaryType::Maj) => ret.nb_maj += 1,
            Buf(s) => {
                if s.is_inverted() {
                    ret.nb_not += 1;
                } else {
                    ret.nb_buf += 1;
                }
            }
            Dff([_, en, res]) => {
                ret.nb_dff += 1;
                if !en.is_constant() {
                    ret.nb_dffe += 1;
                }
                if !res.is_constant() {
                    ret.nb_dffr += 1;
                }
            }
            Nary(v, tp) => match tp {
                NaryType::And | NaryType::Or | NaryType::Nand | NaryType::Nor => {
                    ret.add_and(v.len());
                }
                NaryType::Xor | NaryType::Xnor => {
                    ret.add_xor(v.len());
                }
            },
        }
    }

    ret
}
