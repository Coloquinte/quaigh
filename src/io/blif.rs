use core::panic;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::iter::zip;

use crate::network::{BinaryType, NaryType, TernaryType};
use crate::{Gate, Network, Signal};

use super::utils::{get_inverted_signals, sig_to_string};

enum Statement {
    Inputs(Vec<String>),
    Outputs(Vec<String>),
    Latch { input: String, output: String },
    Name(Vec<String>),
    Cube(String),
}

fn build_name_to_sig(statements: &Vec<Statement>) -> Result<HashMap<String, Signal>, String> {
    let mut ret = HashMap::new();
    let mut var = 0;
    for statement in statements {
        match statement {
            Statement::Inputs(inputs) => {
                for (i, name) in inputs.iter().enumerate() {
                    let s = Signal::from_input(i as u32);
                    let present = ret.insert(name.clone(), s).is_some();
                    if present {
                        return Err(format!("{} is defined twice", name));
                    }
                }
            }
            Statement::Outputs(_) => (),
            Statement::Latch {
                input: _,
                output: name,
            } => {
                let s = Signal::from_var(var as u32);
                var += 1;
                let present = ret.insert(name.clone(), s).is_some();
                if present {
                    return Err(format!("{} is defined twice", name));
                }
            }
            Statement::Name(names) => {
                if names.is_empty() {
                    return Err(".names statement with no output".to_owned());
                }
                let s = Signal::from_var(var as u32);
                let name = names.last().unwrap();
                var += 1;
                let present = ret.insert(name.clone(), s).is_some();
                if present {
                    return Err(format!("{} is defined twice", name));
                }
            }
            Statement::Cube(_) => (),
        }
    }
    Ok(ret)
}

fn build_network(
    statements: &Vec<Statement>,
    name_to_sig: &HashMap<String, Signal>,
) -> Result<Network, String> {
    let mut ret: Network = Network::new();

    let mut names_to_process = Vec::new();

    for (i, statement) in statements.iter().enumerate() {
        match statement {
            Statement::Inputs(inputs) => ret.add_inputs(inputs.len()),
            Statement::Outputs(outputs) => {
                for name in outputs {
                    let s = name_to_sig
                        .get(name)
                        .ok_or_else(|| format!("{} is not defined", name))?;
                    ret.add_output(*s);
                }
            }
            Statement::Latch { input, output: _ } => {
                ret.add(Gate::dff(name_to_sig[input], Signal::one(), Signal::zero()));
            }
            Statement::Name(names) => {
                let mut deps = Vec::new();
                for name in names.iter().take(names.len() - 1) {
                    let s = name_to_sig
                        .get(name)
                        .ok_or_else(|| format!("{} is not defined", name))?;
                    deps.push(*s);
                }
                names_to_process.push((i, ret.nb_nodes()));
                ret.add(Gate::andn(&deps));
            }
            Statement::Cube(_) => (),
        }
    }

    // Now that all gates have been added, we can process cubes that may require adding new gates
    for (i, gate) in names_to_process {
        let inputs = ret.gate(gate).dependencies();
        let mut cubes = Vec::new();
        for j in (i + 1)..statements.len() {
            if let Statement::Cube(s) = &statements[j] {
                cubes.push(s);
            } else {
                break;
            }
        }
        let mut cube_gates = Vec::new();
        let mut polarities = Vec::new();
        for s in cubes {
            let mut deps = Vec::new();
            let t = s.split_whitespace().collect::<Vec<_>>();

            let (cube_inputs, cube_pol) = if t.len() == 2 {
                (t[0].as_bytes(), t[1])
            } else if t.len() == 1 {
                ("".as_bytes(), t[0])
            } else {
                return Err(format!("Invalid cube: {}", s));
            };
            if cube_inputs.len() != inputs.len() {
                return Err(format!(
                    "Invalid cube: {} has {} inputs, expected {}",
                    s,
                    cube_inputs.len(),
                    inputs.len()
                ));
            }
            for (c, s) in zip(cube_inputs, inputs) {
                if *c == '0' as u8 {
                    deps.push(*s);
                } else if *c == '1' as u8 {
                    deps.push(!s);
                } else if *c != '-' as u8 {
                    return Err(format!("Invalid cube: {}", s));
                }
            }
            let pol = match cube_pol {
                "0" => false,
                "1" => true,
                _ => return Err(format!("Invalid cube: {}", s)),
            };
            polarities.push(pol);
            if pol {
                cube_gates.push(Gate::andn(&deps));
            } else {
                cube_gates.push(Gate::Nary(deps.into(), NaryType::Nand));
            }
        }
        if cube_gates.is_empty() {
            ret.replace(gate, Gate::Buf(Signal::zero()));
        } else if cube_gates.len() == 1 {
            ret.replace(gate, cube_gates[0].clone());
        } else {
            for p in &polarities {
                if *p != polarities[0] {
                    return Err("Inconsistent polarities in cubes".to_owned());
                }
            }
            let mut deps = Vec::new();
            for g in cube_gates {
                deps.push(ret.add(g));
            }
            if polarities[0] {
                ret.replace(gate, Gate::Nary(deps.into(), NaryType::Or));
            } else {
                ret.replace(gate, Gate::Nary(deps.into(), NaryType::Nand));
            }
        }
    }
    ret.topo_sort();
    Ok(ret)
}

fn read_statements<R: std::io::Read>(r: R) -> Result<Vec<Statement>, String> {
    let mut ret = Vec::new();
    let mut found_model = false;
    let mut found_inputs = false;
    let mut found_outputs = false;
    for l in BufReader::new(r).lines() {
        if let Ok(s) = l {
            let t = s.trim().to_owned();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let tokens: Vec<_> = t.split_whitespace().collect();

            match tokens[0] {
                ".model" => {
                    if found_model {
                        return Err("Multiple models in the same file are not supported".to_owned());
                    }
                    found_model = true;
                }
                ".inputs" => {
                    if found_inputs {
                        return Err("Multiple .inputs statements".to_owned());
                    }
                    found_inputs = true;
                    ret.push(Statement::Inputs(
                        tokens[1..].iter().map(|s| (*s).to_owned()).collect(),
                    ));
                }
                ".outputs" => {
                    if found_outputs {
                        return Err("Multiple .outputs statements".to_owned());
                    }
                    found_outputs = true;
                    ret.push(Statement::Outputs(
                        tokens[1..].iter().map(|s| (*s).to_owned()).collect(),
                    ));
                }
                ".latch" => {
                    ret.push(Statement::Latch {
                        input: tokens[1].to_owned(),
                        output: tokens[2].to_owned(),
                    });
                }
                ".names" => {
                    ret.push(Statement::Name(
                        tokens[1..].iter().map(|s| (*s).to_owned()).collect(),
                    ));
                }
                ".flop" | ".cname" | ".gate" | ".subckt" => {
                    return Err(format!("{} construct is not supported", tokens[0]));
                }
                ".end" => continue,
                _ => {
                    ret.push(Statement::Cube(t));
                }
            }
        }
    }
    Ok(ret)
}

/// Read a network in .blif format
///
/// The format specification is available [here](https://course.ece.cmu.edu/~ee760/760docs/blif.pdf),
/// with extensions introduced by [ABC](https://people.eecs.berkeley.edu/~alanmi/publications/other/boxes01.pdf)
/// and [Yosys](https://yosyshq.readthedocs.io/projects/yosys/en/latest/cmd/write_blif.html) and
/// [VPR](https://docs.verilogtorouting.org/en/latest/vpr/file_formats/).
///
/// Quaigh only support a small subset, with a single module and a single clock.
pub fn read_blif<R: std::io::Read>(r: R) -> Result<Network, String> {
    let statements = read_statements(r)?;
    let name_to_sig = build_name_to_sig(&statements)?;
    build_network(&statements, &name_to_sig)
}

pub fn write_blif_cube<W: Write>(w: &mut W, mask: usize, num_vars: usize, val: bool) {
    for i in 0..num_vars {
        let val_i = (mask >> i) & 1 != 0;
        write!(w, "{}", if val_i { "1" } else { "0" }).unwrap();
    }
    write!(w, "{}", if val { " 1" } else { " 0" }).unwrap();
}

/// Write a network in .blif format
///
/// The format specification is available [here](https://course.ece.cmu.edu/~ee760/760docs/blif.pdf),
/// with extensions introduced by [ABC](https://people.eecs.berkeley.edu/~alanmi/publications/other/boxes01.pdf)
/// and [Yosys](https://yosyshq.readthedocs.io/projects/yosys/en/latest/cmd/write_blif.html) and
/// [VPR](https://docs.verilogtorouting.org/en/latest/vpr/file_formats/).
///
/// Quaigh only support a small subset, with a single module and a single clock.
pub fn write_blif<W: Write>(w: &mut W, aig: &Network) {
    writeln!(w, "# .blif file").unwrap();
    writeln!(w, "# Generated by quaigh").unwrap();
    writeln!(w, ".model quaigh").unwrap();

    // Write input specifiers
    write!(w, ".inputs").unwrap();
    for i in 0..aig.nb_inputs() {
        write!(w, " {}", aig.input(i)).unwrap();
    }
    writeln!(w).unwrap();

    // Write output specifiers
    write!(w, ".outputs").unwrap();
    for i in 0..aig.nb_outputs() {
        write!(w, " {}", sig_to_string(&aig.output(i))).unwrap();
    }
    writeln!(w).unwrap();

    // Write latches
    for i in 0..aig.nb_nodes() {
        if let Gate::Dff([d, en, res]) = aig.gate(i) {
            if *en != Signal::one() || *res != Signal::zero() {
                // ABC extension to blif
                write!(w, ".flop D={} Q=x{} init=0", sig_to_string(d), i).unwrap();
                if *en != Signal::one() {
                    write!(w, " E={}", en).unwrap();
                }
                if *res != Signal::zero() {
                    write!(w, " R={}", en).unwrap();
                }
                writeln!(w).unwrap();
            } else {
                writeln!(w, ".latch {} x{} 0", sig_to_string(d), i).unwrap();
            }
        }
    }
    writeln!(w).unwrap();

    // Write gates
    for i in 0..aig.nb_nodes() {
        let g = aig.gate(i);
        if !g.is_comb() {
            continue;
        }
        write!(w, ".names").unwrap();
        if let Gate::Buf(s) = g {
            // Buffers handle the inversions themselves
            write!(w, " {}", sig_to_string(&s.without_inversion())).unwrap();
        } else {
            // Other signals use a buffered signal for inverted inputs
            for s in g.dependencies() {
                write!(w, " {}", sig_to_string(s)).unwrap();
            }
        }
        writeln!(w, " x{}", i).unwrap();

        match g {
            Gate::Binary(_, BinaryType::And) => {
                writeln!(w, "11 1").unwrap();
            }
            Gate::Binary(_, BinaryType::Xor) => {
                writeln!(w, "10 1").unwrap();
                writeln!(w, "01 1").unwrap();
            }
            Gate::Ternary(_, TernaryType::And) => {
                writeln!(w, "111 1").unwrap();
            }
            Gate::Ternary(_, TernaryType::Xor) => {
                writeln!(w, "111 1").unwrap();
                writeln!(w, "100 1").unwrap();
                writeln!(w, "010 1").unwrap();
                writeln!(w, "001 1").unwrap();
            }
            Gate::Ternary(_, TernaryType::Mux) => {
                writeln!(w, "11- 1").unwrap();
                writeln!(w, "0-1 1").unwrap();
            }
            Gate::Ternary(_, TernaryType::Maj) => {
                writeln!(w, "11- 1").unwrap();
                writeln!(w, "-11 1").unwrap();
                writeln!(w, "1-1 1").unwrap();
            }
            Gate::Nary(v, tp) => {
                if matches!(
                    tp,
                    NaryType::And | NaryType::Nand | NaryType::Nor | NaryType::Or
                ) {
                    let input_inv = matches!(tp, NaryType::Nor | NaryType::Or);
                    let output_inv = matches!(tp, NaryType::Or | NaryType::Nand);
                    for _ in 0..v.len() {
                        if input_inv {
                            write!(w, "0").unwrap();
                        } else {
                            write!(w, "1").unwrap();
                        }
                    }
                    if output_inv {
                        writeln!(w, " 0").unwrap();
                    } else {
                        writeln!(w, " 1").unwrap();
                    }
                } else {
                    for mask in 0usize..(1 << v.len()) {
                        let xor_val = mask.count_ones() % 2 != 0;
                        let val = match tp {
                            NaryType::Xor => xor_val,
                            NaryType::Xnor => !xor_val,
                            _ => unreachable!(),
                        };
                        if val {
                            write_blif_cube(w, mask, v.len(), val);
                        }
                    }
                }
            }
            Gate::Buf(s) => {
                if s.is_inverted() {
                    writeln!(w, "0 1").unwrap();
                } else {
                    writeln!(w, "1 1").unwrap();
                }
            }
            Gate::Lut(lut) => {
                for mask in 0..lut.lut.num_bits() {
                    let val = lut.lut.value(mask);
                    if val {
                        write_blif_cube(w, mask, lut.lut.num_vars(), val);
                    }
                }
            }
            _ => panic!("Gate type not supported"),
        }
    }

    // Write inverters
    let signals_with_inv = get_inverted_signals(aig);
    for s in signals_with_inv {
        writeln!(w, ".names {} {}_n", s, s).unwrap();
        writeln!(w, "0 1").unwrap();
    }

    // Write constants
    writeln!(w, ".names vdd").unwrap();
    writeln!(w, "1").unwrap();
    writeln!(w, ".names gnd").unwrap();
}
