use core::fmt;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::network::gates::{Gate, Normalization};
use crate::network::signal::Signal;

/// Representation of a logic network as a gate-inverter-graph, used as the main representation for all logic manipulations
#[derive(Debug, Clone, Default)]
pub struct Network {
    nb_inputs: usize,
    nodes: Vec<Gate>,
    outputs: Vec<Signal>,
}

impl Network {
    /// Create a new network
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the number of primary inputs
    pub fn nb_inputs(&self) -> usize {
        self.nb_inputs
    }

    /// Return the number of primary outputs
    pub fn nb_outputs(&self) -> usize {
        self.outputs.len()
    }

    /// Return the number of nodes in the network
    pub fn nb_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get the input at index i
    pub fn input(&self, i: usize) -> Signal {
        assert!(i < self.nb_inputs());
        Signal::from_input(i as u32)
    }

    /// Get the output at index i
    pub fn output(&self, i: usize) -> Signal {
        assert!(i < self.nb_outputs());
        self.outputs[i]
    }

    /// Get the variable at index i
    pub fn node(&self, i: usize) -> Signal {
        Signal::from_var(i as u32)
    }

    /// Get the gate at index i
    pub fn gate(&self, i: usize) -> &Gate {
        &self.nodes[i]
    }

    /// Add a new primary input
    pub fn add_input(&mut self) -> Signal {
        self.nb_inputs += 1;
        self.input(self.nb_inputs() - 1)
    }

    /// Add multiple primary inputs
    pub fn add_inputs(&mut self, nb: usize) {
        self.nb_inputs += nb;
    }

    /// Add a new primary output based on an existing literal
    pub fn add_output(&mut self, l: Signal) {
        self.outputs.push(l)
    }

    /// Create an And2 gate
    pub fn and(&mut self, a: Signal, b: Signal) -> Signal {
        self.add_canonical(Gate::and(a, b))
    }

    /// Create a Xor2 gate
    pub fn xor(&mut self, a: Signal, b: Signal) -> Signal {
        self.add_canonical(Gate::xor(a, b))
    }

    /// Create a Dff gate (flip flop)
    pub fn dff(&mut self, data: Signal, enable: Signal, reset: Signal) -> Signal {
        self.add_canonical(Gate::dff(data, enable, reset))
    }

    /// Add a new gate, and make it canonical. The gate may be simplified immediately
    pub fn add_canonical(&mut self, gate: Gate) -> Signal {
        use Normalization::*;
        let g = gate.make_canonical();
        match g {
            Copy(l) => l,
            Node(g, inv) => self.add(g) ^ inv,
        }
    }

    /// Add a new gate
    pub fn add(&mut self, gate: Gate) -> Signal {
        let l = Signal::from_var(self.nodes.len() as u32);
        self.nodes.push(gate);
        l
    }

    /// Replace an existing gate
    pub fn replace(&mut self, i: usize, gate: Gate) -> Signal {
        let l = Signal::from_var(i as u32);
        self.nodes[i] = gate;
        l
    }

    /// Return whether the network is purely combinatorial
    pub fn is_comb(&self) -> bool {
        self.nodes.iter().all(|g| g.is_comb())
    }

    /// Return whether the network is already topologically sorted (except for flip-flops)
    pub(crate) fn is_topo_sorted(&self) -> bool {
        for (i, g) in self.nodes.iter().enumerate() {
            let ind = i as u32;
            if g.is_comb() {
                for v in g.vars() {
                    if v >= ind {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Remap nodes; there may be holes in the translation
    fn remap(&mut self, order: &[u32]) -> Box<[Signal]> {
        // Create the translation
        let mut translation = vec![Signal::zero(); self.nb_nodes()];
        for (new_i, old_i) in order.iter().enumerate() {
            translation[*old_i as usize] = Signal::from_var(new_i as u32);
        }

        // Remap the nodes
        let mut new_nodes = Vec::new();
        for o in order {
            let i = *o as usize;
            let g = self.gate(i);
            assert!(translation[i].is_var());
            assert_eq!(translation[i].var(), new_nodes.len() as u32);
            new_nodes.push(g.remap_order(translation.as_slice()));
        }
        self.nodes = new_nodes;

        // Remap the outputs
        self.remap_outputs(&translation);
        translation.into()
    }

    /// Shuffle the network randomly; this will invalidate all signals
    ///
    /// Returns the mapping of old variable indices to signals, if needed.
    pub fn shuffle(&mut self, seed: u64) -> Box<[Signal]> {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let mut order: Vec<u32> = (0..self.nb_nodes() as u32).collect();
        order.shuffle(&mut rng);
        self.remap(&order);
        self.topo_sort()
    }

    /// Remap outputs
    fn remap_outputs(&mut self, translation: &[Signal]) {
        let new_outputs = self
            .outputs
            .iter()
            .map(|s| s.remap_order(translation))
            .collect();
        self.outputs = new_outputs;
    }

    /// Remove unused logic; this will invalidate all signals
    ///
    /// Returns the mapping of old variable indices to signals, if needed.
    /// Removed signals are mapped to zero.
    pub fn cleanup(&mut self) -> Box<[Signal]> {
        // Mark unused logic
        let mut visited = vec![false; self.nb_nodes()];
        let mut to_visit = Vec::<u32>::new();
        for o in 0..self.nb_outputs() {
            let output = self.output(o);
            if output.is_var() {
                to_visit.push(output.var());
            }
        }
        while !to_visit.is_empty() {
            let node = to_visit.pop().unwrap() as usize;
            if visited[node] {
                continue;
            }
            visited[node] = true;
            to_visit.extend(self.gate(node).vars());
        }

        // Now compute a mapping for all nodes that are reachable
        let mut order = Vec::new();
        for (i, v) in visited.iter().enumerate() {
            if *v {
                order.push(i as u32);
            }
        }
        self.remap(order.as_slice())
    }

    /// Remove duplicate logic and make all gates canonical; this will invalidate all signals
    ///
    /// Canonical gates are And, Xor, Mux, Maj and Lut. Everything else will be simplified.
    /// Returns the mapping of old variable indices to signals, if needed.
    pub fn make_canonical(&mut self) -> Box<[Signal]> {
        self.dedup(true)
    }

    /// Remove duplicate logic; this will invalidate all signals
    ///
    /// Returns the mapping of old variable indices to signals, if needed.
    pub fn deduplicate(&mut self) -> Box<[Signal]> {
        self.dedup(false)
    }

    /// Remove duplicate logic. Optionally make all gates canonical
    fn dedup(&mut self, make_canonical: bool) -> Box<[Signal]> {
        // Replace each node, in turn, by a simplified version or an equivalent existing node
        // We need the network to be topologically sorted, so that the gate inputs are already replaced
        // Dff gates are an exception to the sorting, and are handled separately
        assert!(self.is_topo_sorted());
        let mut translation = (0..self.nb_nodes())
            .map(|i| Signal::from_var(i as u32))
            .collect::<Vec<Signal>>();

        /// Core function for deduplication
        fn dedup_node(
            g: &Gate,
            h: &mut HashMap<Gate, Signal>,
            nodes: &mut Vec<Gate>,
            make_canonical: bool,
        ) -> Signal {
            let normalized = if make_canonical {
                g.make_canonical()
            } else {
                Normalization::Node(g.clone(), false)
            };
            match normalized {
                Normalization::Copy(sig) => sig,
                Normalization::Node(g, inv) => {
                    let node_s = Signal::from_var(nodes.len() as u32);
                    match h.entry(g.clone()) {
                        Entry::Occupied(e) => e.get() ^ inv,
                        Entry::Vacant(e) => {
                            e.insert(node_s);
                            nodes.push(g);
                            node_s ^ inv
                        }
                    }
                }
            }
        }

        // TODO: use faster hashing + do not own the Gate
        let mut hsh = HashMap::new();
        let mut new_nodes = Vec::new();

        // Dedup flip flops
        for i in 0..self.nb_nodes() {
            let g = self.gate(i);
            if !g.is_comb() {
                translation[i] = dedup_node(g, &mut hsh, &mut new_nodes, make_canonical);
            }
        }

        // Remap and dedup combinatorial gates
        for i in 0..self.nb_nodes() {
            let g = self.gate(i).remap_order(translation.as_slice());
            if g.is_comb() {
                translation[i] = dedup_node(&g, &mut hsh, &mut new_nodes, make_canonical);
            }
        }

        // Remap flip flops
        for i in 0..new_nodes.len() {
            if !new_nodes[i].is_comb() {
                new_nodes[i] = new_nodes[i].remap_order(translation.as_slice());
            }
        }

        self.nodes = new_nodes;
        self.remap_outputs(&translation);
        self.check();
        translation.into()
    }

    /// Topologically sort the network; this will invalidate all signals
    ///
    /// Ordering may be changed even if already sorted. Flip-flop ordering is kept as is.
    /// Returns the mapping of old variable indices to signals, if needed.
    pub(crate) fn topo_sort(&mut self) -> Box<[Signal]> {
        // Count the output dependencies of each gate
        let mut count_deps = vec![0u32; self.nb_nodes()];
        for g in self.nodes.iter() {
            if g.is_comb() {
                for v in g.vars() {
                    count_deps[v as usize] += 1;
                }
            }
        }

        // Compute the topological sort
        let mut rev_order: Vec<u32> = Vec::new();
        let mut visited = vec![false; self.nb_nodes()];

        // Handle Dff separately so they are not reordered
        for i in 0..self.nb_nodes() {
            if !self.gate(i).is_comb() {
                visited[i] = true;
            }
        }

        // Start with gates with no dependencies
        let mut to_visit: Vec<u32> = (0..self.nb_nodes())
            .filter(|v| count_deps[*v] == 0 && !visited[*v])
            .map(|v| v as u32)
            .collect();
        while let Some(v) = to_visit.pop() {
            // TODO: allow for some randomness here
            // Visit the gate and mark the gates with satisfied dependencies
            if visited[v as usize] {
                continue;
            }
            visited[v as usize] = true;
            rev_order.push(v);
            let g = self.gate(v as usize);
            if g.is_comb() {
                for d in g.vars() {
                    count_deps[d as usize] -= 1;
                    if count_deps[d as usize] == 0 {
                        to_visit.push(d);
                    }
                }
            }
        }

        // Add Dff first to the order (first, so last in the reversed order)
        for i in (0..self.nb_nodes()).rev() {
            if !self.gate(i).is_comb() {
                rev_order.push(i as u32);
            }
        }

        if rev_order.len() != self.nb_nodes() {
            panic!("Unable to find a valid topological sort: there must be a combinatorial loop");
        }
        rev_order.reverse();
        let order = rev_order;

        self.remap(order.as_slice())
    }

    /// Check consistency of the datastructure
    pub fn check(&self) {
        for i in 0..self.nb_nodes() {
            for v in self.gate(i).dependencies() {
                assert!(self.is_valid(*v), "Invalid signal {v}");
            }
        }
        for i in 0..self.nb_outputs() {
            let v = self.output(i);
            assert!(self.is_valid(v), "Invalid output {v}");
        }
        assert!(self.is_topo_sorted());
    }

    /// Returns whether a signal is valid (within bounds) in the network
    pub(crate) fn is_valid(&self, s: Signal) -> bool {
        if s.is_input() {
            s.input() < self.nb_inputs() as u32
        } else if s.is_var() {
            s.var() < self.nb_nodes() as u32
        } else {
            true
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Network with {} inputs, {} outputs:",
            self.nb_inputs(),
            self.nb_outputs()
        )?;
        for i in 0..self.nb_nodes() {
            writeln!(f, "\t{} = {}", self.node(i), self.gate(i))?;
        }
        for i in 0..self.nb_outputs() {
            writeln!(f, "\to{} = {}", i, self.output(i))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Gate, Network, Signal};

    #[test]
    fn test_basic() {
        let mut aig = Network::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let x = aig.xor(i0, i1);
        aig.add_output(x);

        // Basic properties
        assert_eq!(aig.nb_inputs(), 2);
        assert_eq!(aig.nb_outputs(), 1);
        assert_eq!(aig.nb_nodes(), 1);
        assert!(aig.is_comb());
        assert!(aig.is_topo_sorted());

        // Access
        assert_eq!(aig.input(0), i0);
        assert_eq!(aig.input(1), i1);
        assert_eq!(aig.output(0), x);
    }

    #[test]
    fn test_dff() {
        let mut aig = Network::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let c0 = Signal::zero();
        let c1 = Signal::one();
        // Useful Dff
        assert_eq!(aig.dff(i0, i1, i2), Signal::from_var(0));
        assert_eq!(aig.dff(i0, i1, c0), Signal::from_var(1));
        // Dff that reduces to 0
        assert_eq!(aig.dff(c0, i1, i2), c0);
        assert_eq!(aig.dff(i0, c0, i2), c0);
        assert_eq!(aig.dff(i0, i1, c1), c0);
        assert!(!aig.is_comb());
        assert!(aig.is_topo_sorted());
    }

    #[test]
    fn test_sweep() {
        let mut aig = Network::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let x0 = aig.and(i0, i1);
        let x1 = !aig.and(!i0, !i1);
        let _ = aig.and(x0, i1);
        let x3 = !aig.and(!x1, !i1);
        aig.add_output(x3);
        let t = aig.cleanup();
        assert_eq!(t.len(), 4);
        assert_eq!(aig.nb_nodes(), 2);
        assert_eq!(aig.nb_outputs(), 1);
        assert_eq!(
            t,
            vec![
                Signal::zero(),
                Signal::from_var(0),
                Signal::zero(),
                Signal::from_var(1)
            ]
            .into()
        );
    }

    #[test]
    fn test_dedup() {
        let mut aig = Network::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let x0 = aig.and(i0, i1);
        let x0_s = aig.and(i0, i1);
        let x1 = aig.and(x0, i2);
        let x1_s = aig.and(x0_s, i2);
        aig.add_output(x1);
        aig.add_output(x1_s);
        aig.make_canonical();
        assert_eq!(aig.nb_nodes(), 2);
    }

    #[test]
    fn test_topo_sort() {
        let mut aig = Network::default();
        let i0 = aig.add_input();
        let i1 = aig.add_input();
        let i2 = aig.add_input();
        let x0 = Gate::dff(i2, Signal::one(), Signal::zero());
        let x1 = Gate::dff(i1, Signal::one(), Signal::zero());
        let x2 = Gate::dff(i0, Signal::one(), Signal::zero());
        let x3 = Gate::dff(i2, i1, Signal::zero());
        aig.add(x0.clone());
        aig.add(x1.clone());
        aig.add(x2.clone());
        aig.add(x3.clone());
        aig.topo_sort();
        assert_eq!(aig.nb_nodes(), 4);
        assert_eq!(aig.gate(0), &x0);
        assert_eq!(aig.gate(1), &x1);
        assert_eq!(aig.gate(2), &x2);
        assert_eq!(aig.gate(3), &x3);
    }
}
