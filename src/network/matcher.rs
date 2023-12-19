//! Simple pattern matching to perform search/replace on logic networks

use std::iter::zip;

use crate::{Gate, Network, Signal};

/// Pattern matching algorithm
///
/// This will find a correspondence between signals in the pattern and signals in the network,
/// starting from an anchor gate.
/// Patterns allow to use signals multiple times and can even have loops.
/// Each signal in the pattern will match one signal in the network, but a signal in the network
/// can be matched multiple times: pattern i0 & i1 will match both xi & xj and xi & xi.
///
/// Variable length patterns are not supported. For example, there is no way to match a chain of
/// buffers of arbitrary length or a gate with an arbitrary number of inputs, but you can make
/// a pattern for a fixed length.
///
/// Input order matters. a & (b & c) is a different pattern from (a & b) & c.
pub struct Matcher<'a> {
    matches: Vec<Signal>,
    pattern: &'a Network,
}

impl<'a> Matcher<'a> {
    /// Build the pattern matcher from a pattern
    pub fn from_pattern(pattern: &Network) -> Matcher {
        let matches = vec![Signal::placeholder(); pattern.nb_inputs() + pattern.nb_nodes()];
        assert!(pattern.nb_outputs() == 1);
        assert!(!pattern.output(0).is_inverted());
        assert!(!pattern.nb_nodes() >= 1);
        // TODO: check that the pattern has a path from output to all inputs and internal gates
        Matcher { matches, pattern }
    }

    /// Run the pattern matching algorithm on the given gate. Returns the matched inputs, if any
    pub fn matches(&mut self, aig: &Network, i: usize) -> Option<Vec<Signal>> {
        let matched = self.try_match(self.pattern.output(0), aig, Signal::from_var(i as u32));
        let ret = if matched {
            let v = (0..self.pattern.nb_inputs())
                .map(|i| self.get_match(Signal::from_input(i as u32)))
                .collect();
            Some(v)
        } else {
            None
        };
        self.reset();
        ret
    }

    /// Core recursive function for the pattern matching
    ///
    /// It works as follows:
    ///   * Check whether the signal is already matched, and returns if a mismatch is found
    ///   * Check that the gate types match
    ///   * Call recursively on each gate input
    fn try_match(&mut self, repr: Signal, aig: &Network, s: Signal) -> bool {
        let existing_match = self.get_match(repr);
        if existing_match != Signal::placeholder() {
            return existing_match == s;
        }
        self.set_match(repr, s);
        if repr.is_var() {
            // Match a gate
            if !s.is_var() {
                return false;
            }
            // Needs to be used with the same polarity
            if s.is_inverted() != repr.is_inverted() {
                return false;
            }
            let g_repr = self.pattern.gate(repr.var() as usize);
            let g = aig.gate(s.var() as usize);
            if !Matcher::gate_type_matches(g_repr, g) {
                return false;
            }
            for (&repr_r, &s_r) in zip(g_repr.dependencies(), g.dependencies()) {
                if !self.try_match(repr_r, aig, s_r) {
                    return false;
                }
            }
            true
        } else if repr.is_input() {
            true
        } else {
            // Constant
            repr == s
        }
    }

    /// Check whether a gate type matches
    fn gate_type_matches(g_repr: &Gate, g: &Gate) -> bool {
        use Gate::*;
        match (g_repr, g) {
            (Binary(_, t1), Binary(_, t2)) => t1 == t2,
            (Ternary(_, t1), Ternary(_, t2)) => t1 == t2,
            (Nary(v1, t1), Nary(v2, t2)) => t1 == t2 && v1.len() == v2.len(),
            (Buf(_), Buf(_)) => true,
            (Dff(_), Dff(_)) => true,
            _ => false,
        }
    }

    /// Get the signal currently matched to a given pattern signal
    fn get_match(&self, repr: Signal) -> Signal {
        if repr.is_constant() {
            return repr;
        }
        let ind = if repr.is_input() {
            repr.input() as usize
        } else {
            self.pattern.nb_inputs() + repr.var() as usize
        };
        let m = self.matches[ind];
        if m == Signal::placeholder() {
            m
        } else {
            m ^ repr.is_inverted()
        }
    }

    /// Set the signal currently matched to a given pattern signal
    fn set_match(&mut self, repr: Signal, val: Signal) {
        assert!(!repr.is_constant());
        let ind = if repr.is_input() {
            repr.input() as usize
        } else {
            self.pattern.nb_inputs() + repr.var() as usize
        };
        self.matches[ind] = val ^ repr.is_inverted();
    }

    /// Reset the internal state, putting all signals to placeholder
    fn reset(&mut self) {
        for m in &mut self.matches {
            *m = Signal::placeholder();
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{Gate, Network, Signal};

    use super::Matcher;

    /// Test single gate pattern matching on and gates
    #[test]
    fn test_and() {
        let mut aig = Network::new();
        aig.add_inputs(3);
        let i0 = Signal::from_input(0);
        let i1 = Signal::from_input(1);
        let i2 = Signal::from_input(2);
        aig.add(Gate::and(i0, i1));
        aig.add(Gate::and(i0, i2));
        aig.add(Gate::and(i2, i1));
        aig.add(Gate::and(i0, !i1));
        aig.add(Gate::and(!i0, i1));
        aig.add(Gate::xor(i0, i1));
        aig.add(Gate::xor(!i0, i1));

        let mut pattern = Network::new();
        pattern.add_inputs(2);
        let o = pattern.add(Gate::and(i0, i1));
        pattern.add_output(o);

        let mut matcher = Matcher::from_pattern(&pattern);
        for i in 0..5 {
            assert!(matcher.matches(&aig, i).is_some());
        }
        for i in 5..7 {
            assert!(matcher.matches(&aig, i).is_none());
        }

        assert_eq!(matcher.matches(&aig, 0), Some(vec![i0, i1]));
        assert_eq!(matcher.matches(&aig, 1), Some(vec![i0, i2]));
        assert_eq!(matcher.matches(&aig, 2), Some(vec![i2, i1]));
        assert_eq!(matcher.matches(&aig, 3), Some(vec![i0, !i1]));
        assert_eq!(matcher.matches(&aig, 4), Some(vec![!i0, i1]));
    }

    /// Test more complex pattern matching
    #[test]
    fn test_complex_xor() {
        let mut aig = Network::new();
        aig.add_inputs(2);
        let i0 = Signal::from_input(0);
        let i1 = Signal::from_input(1);
        let x0 = aig.add(Gate::and(i0, !i1));
        let x1 = aig.add(Gate::and(!i0, i1));
        aig.add(Gate::and(!x0, !x1));
        aig.add(Gate::and(x0, x1));
        aig.add(Gate::and(!x0, x1));
        aig.add(Gate::and(!x1, !x0));

        let mut pattern = Network::new();
        pattern.add_inputs(2);
        let p0 = pattern.add(Gate::and(i0, !i1));
        let p1 = pattern.add(Gate::and(!i0, i1));
        let o = pattern.add(Gate::and(!p0, !p1));
        pattern.add_output(o);

        let mut matcher = Matcher::from_pattern(&pattern);
        assert_eq!(matcher.matches(&aig, 2), Some(vec![i0, i1]));
        assert_eq!(matcher.matches(&aig, 3), None);
        assert_eq!(matcher.matches(&aig, 4), None);
        assert_eq!(matcher.matches(&aig, 5), Some(vec![!i0, !i1]));
    }
}
