
This file lists useful developments ideas for Quaigh.


# Optimization and transformations

## 2-level simplification

2-level simplification of Sum-of-Products and Exclusive-Sum-of-Products is very common and useful.
Ideally this would take the sharing of And gates into account, which is not usually done.

Types of 2-level simplification to handle:
*   Sum of Products (SoP): most typical
*   Exclusive Sum of Products (ESoP): common too
*   Sum of Exclusive Sums (SoES): not a general model, but would be useful

Exact algorithms:
*   For SoP and SoES: enumerate maximum cubes, then solve a variant of minimum set cover problem
*   For ESoP, enumerate all cubes, then solve a xor-constrained minimization problem

## Local rewriting

The typical approach to local rewriting is with a dictionary of "optimal" 4-input or 5-input functions.
I'd like multi-output local rewriting instead, using a low-depth dictionary of common functions.

## AIG/MIG transformation

Simple transformation to go back to an And-based or Mux-based view.


# Technology mapping

## Cut enumeration for FPGAs

Cut enumeration is necessary for any FPGA techmapping. I'd like ours to go a bit further and include Dff in the Cuts.

## Techmapping API

Technology mapping is "just" a question of dependencies between cuts, which each have their own area and delay.
Solving it can be almost completely separate from cut enumeration.

This paves the way for additional optimizations:
*   techmapping for FPGA and ASIC can be shared
*   multiple choices can be exposed without having them in the logic: N-input gates can be cut a number of ways


# Test pattern generation

## Input stuck-at faults

Only output stuck-at faults are implemented. This would be a simple addition for better coverage.

## Path activation

We want to be able to activate whole critical paths. This is a bit more complicated to grasp.

## Redundant fault removal

Remove faults that are redundant, such as input/output of a unary gate.

## Faster simulation

The current simulation is OK but basic.
A transformation to an And or Mux graph and a single array for indexing would go a long way to make it faster.
On the other hand this requires an additional translation layer.

## Incremental simulation

Use a queue to only simulate the part that is impacted by a fault.

## Connected components

Partition the circuit in order to handle disjoint parts separately.

