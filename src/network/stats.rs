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

use crate::network::gates::{BinaryType, NaryType, TernaryType};
use crate::{Gate, Network};

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
    /// Number of Lut and similar gates
    pub nb_lut: usize,
    /// Arity of Lut gates
    pub lut_arity: Vec<usize>,
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
        self.nb_and
            + self.nb_xor
            + self.nb_lut
            + self.nb_mux
            + self.nb_maj
            + self.nb_buf
            + self.nb_dff
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

    /// Record a new lut
    fn add_lut(&mut self, sz: usize) {
        self.nb_lut += 1;
        while self.lut_arity.len() <= sz {
            self.lut_arity.push(0);
        }
        self.lut_arity[sz] += 1;
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
                writeln!(f, "      enable: {}", self.nb_dffe)?;
            }
            if self.nb_dffr != 0 {
                writeln!(f, "      reset: {}", self.nb_dffr)?;
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
        if self.nb_lut != 0 {
            writeln!(f, "  Lut: {}", self.nb_lut)?;
            for (i, nb) in self.lut_arity.iter().enumerate() {
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
        nb_lut: 0,
        lut_arity: Vec::new(),
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
                if !s.is_constant() {
                    // Do not count buffered constants that may be created for I/O
                    if s.is_inverted() {
                        ret.nb_not += 1;
                    } else {
                        ret.nb_buf += 1;
                    }
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
            Lut(lut) => {
                ret.add_lut(lut.inputs.len());
            }
        }
    }

    ret
}

/// Count how many times each gate is used, including as output
pub fn count_gate_usage(aig: &Network) -> Vec<usize> {
    let mut ret = vec![0; aig.nb_nodes()];
    for i in 0..aig.nb_nodes() {
        for j in aig.gate(i).vars() {
            ret[j as usize] += 1;
        }
    }
    for i in 0..aig.nb_outputs() {
        let s = aig.output(i);
        if s.is_var() {
            ret[s.var() as usize] += 1;
        }
    }
    ret
}

/// Return which gates use each gate
pub fn gate_users(aig: &Network) -> Vec<Vec<usize>> {
    let mut ret = vec![vec![]; aig.nb_nodes()];
    for i in 0..aig.nb_nodes() {
        for j in aig.gate(i).vars() {
            ret[j as usize].push(i);
        }
    }
    ret
}

/// Mark whether each gate is an output
pub fn gate_is_output(aig: &Network) -> Vec<bool> {
    let mut ret = vec![false; aig.nb_nodes()];
    for i in 0..aig.nb_outputs() {
        if aig.output(i).is_var() {
            ret[aig.output(i).var() as usize] = true;
        }
    }
    ret
}

/// Compute the combinational logic level of every node
///
/// Primary inputs, constants and flip-flop outputs are level 0; each combinatorial
/// gate is one level above its highest input. Buffers and inverters do not add a level.
/// The network must be topologically sorted.
pub fn levels(aig: &Network) -> Vec<u32> {
    let mut lvl = vec![0u32; aig.nb_nodes()];
    for i in 0..aig.nb_nodes() {
        let g = aig.gate(i);
        if !g.is_comb() {
            // A flip-flop output is a sequential source at level 0
            continue;
        }
        let mut m = 0;
        for v in g.vars() {
            m = m.max(lvl[v as usize]);
        }
        lvl[i] = if g.is_buf_like() { m } else { m + 1 };
    }
    lvl
}

/// Compute the combinational depth of the network: the largest logic level over all outputs
///
/// The network must be topologically sorted.
pub fn depth(aig: &Network) -> usize {
    let lvl = levels(aig);
    let mut d = 0;
    for i in 0..aig.nb_outputs() {
        let s = aig.output(i);
        if s.is_var() {
            d = d.max(lvl[s.var() as usize]);
        }
    }
    d as usize
}

#[cfg(test)]
mod tests {
    use volute::Lut3;

    use super::{depth, stats};
    use crate::{Gate, Network, Signal};

    #[test]
    fn test_dff_enable_reset_counts() {
        let mut aig = Network::new();
        let d = aig.add_input();
        let en = aig.add_input();
        let res = aig.add_input();
        // plain dff, dff with enable, dff with enable and reset
        let q0 = aig.dff(d, Signal::one(), Signal::zero());
        let q1 = aig.dff(d, en, Signal::zero());
        let q2 = aig.dff(d, en, res);
        aig.add_output(q0);
        aig.add_output(q1);
        aig.add_output(q2);

        let st = stats(&aig);
        assert_eq!(st.nb_dff, 3);
        assert_eq!(st.nb_dffe, 2);
        assert_eq!(st.nb_dffr, 1);

        // The Display must report the dedicated counts, not nb_dff
        let shown = format!("{st}");
        assert!(shown.contains("enable: 2"), "{shown}");
        assert!(shown.contains("reset: 1"), "{shown}");
    }

    #[test]
    fn test_lut_counts_as_gate() {
        let mut aig = Network::new();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let o = aig.add(Gate::lut(&[i0, i1, i2], Lut3::nth_var(0).into()));
        aig.add_output(o);

        let st = stats(&aig);
        assert_eq!(st.nb_lut, 1);
        assert_eq!(st.nb_gates(), 1);
    }

    #[test]
    fn test_depth() {
        let mut aig = Network::new();
        // A right-leaning And chain of 8 inputs has depth 7
        let mut sigs = Vec::new();
        for _ in 0..8 {
            sigs.push(aig.add_input());
        }
        let mut acc = sigs[0];
        for s in &sigs[1..] {
            acc = aig.and(acc, *s);
        }
        aig.add_output(acc);
        assert_eq!(depth(&aig), 7);

        // A network whose only output is a primary input has depth 0
        let mut io = Network::new();
        let i = io.add_input();
        io.add_output(i);
        assert_eq!(depth(&io), 0);
    }
}
