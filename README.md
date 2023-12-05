<!-- cargo-rdme start -->

Logic simplification and analysis tools

This crate provides tools for logic optimization, synthesis, technology mapping and analysis.
Our goal is to provide an easy-to-use library, and improve its quality over time to match industrial tools.

# Design

Quaigh main representation uses a typical Gate-Inverter-Graph to represent a logic circuit.
Gates are represented explicitly, and inverters are implicit, occupying just one bit.

To make interoperability and optimization easier, many kinds of logic are supported:
* Complex gates such as Xor, Mux and Maj3 are all first class citizens and can coexist in the same circuit;
* Flip-flops with enable and reset are represented directly, not as primary inputs and outputs.

# Usage

Quaigh features bounded equivalence checking, logic simplification and ATPG.
More features will be added over time.
At the moment, logic simplification is far from state of the art: for production designs, you should
generally stick to the tools included in [Yosys](https://github.com/YosysHQ/yosys).

```bash
# Show available commands
# At the moment, only .bench files are supported
quaigh help
# Optimize the logic
quaigh opt mydesign.bench -o optimized.bench
# Check equivalence between the two
quaigh equiv mydesign.bench optimized.bench
# Generate test patterns for the optimized design
quaigh atpg optimized.bench -o atpg.test
```

Quaigh is not published on crates.io yet, but you can install it from the git repository using Cargo:
```bash
cargo install --git https://github.com/Coloquinte/quaigh
```

<!-- cargo-rdme end -->
