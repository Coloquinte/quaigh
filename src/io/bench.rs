//! IO for .bench (ISCAS) files

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

use volute::Lut;

use crate::network::{BinaryType, NaryType, TernaryType};
use crate::{Gate, Network, Signal};

use super::utils::{get_inverted_signals, sig_to_string};

fn build_name_to_sig(
    statements: &Vec<Vec<String>>,
    inputs: &Vec<String>,
) -> HashMap<String, Signal> {
    let mut ret = HashMap::new();
    for (i, name) in inputs.iter().enumerate() {
        let present = ret
            .insert(name.clone(), Signal::from_input(i as u32))
            .is_some();
        assert!(!present, "{} is defined twice", name)
    }
    for (i, s) in statements.iter().enumerate() {
        let present = ret
            .insert(s[0].to_string(), Signal::from_var(i as u32))
            .is_some();
        assert!(!present, "{} is defined twice", s[0].to_string())
    }

    // ABC-style naming for constant signals
    if !ret.contains_key("vdd") {
        ret.insert("vdd".to_string(), Signal::one());
    }
    if !ret.contains_key("gnd") {
        ret.insert("gnd".to_string(), Signal::zero());
    }
    ret
}

fn check_statement(statement: &Vec<String>, name_to_sig: &HashMap<String, Signal>) {
    let deps = &statement[2..];
    for dep in deps {
        assert!(
            name_to_sig.contains_key(dep),
            "Gate input {dep} is not generated anywhere"
        );
    }
    match statement[1].to_uppercase().as_str() {
        "DFF" | "BUF" | "BUFF" | "NOT" => assert_eq!(deps.len(), 1),
        "VDD" | "VSS" => assert_eq!(deps.len(), 0),
        "MUX" | "MAJ" => assert_eq!(deps.len(), 3),
        _ => (),
    };
}

fn gate_dependencies(
    statement: &Vec<String>,
    name_to_sig: &HashMap<String, Signal>,
) -> Box<[Signal]> {
    statement[2..].iter().map(|n| name_to_sig[n]).collect()
}

fn network_from_statements(
    statements: &Vec<Vec<String>>,
    inputs: &Vec<String>,
    outputs: &Vec<String>,
) -> Result<Network, String> {
    let mut ret = Network::new();
    ret.add_inputs(inputs.len());

    // Compute a mapping between the two
    let name_to_sig = build_name_to_sig(statements, inputs);

    // Check everything
    for statement in statements {
        check_statement(statement, &name_to_sig);
    }
    for output in outputs {
        assert!(
            name_to_sig.contains_key(output),
            "Output {output} is not generated anywhere"
        );
    }

    // Setup the variables based on the mapping
    for s in statements {
        let sigs: Box<[Signal]> = gate_dependencies(s, &name_to_sig);
        match s[1].to_uppercase().as_str() {
            "DFF" => {
                ret.add(Gate::Dff([sigs[0], Signal::one(), Signal::zero()]));
            }
            "DFFRSE" => {
                assert_eq!(sigs[1], Signal::zero());
                ret.add(Gate::Dff([sigs[0], sigs[3], sigs[1]]));
            }
            "BUF" | "BUFF" => {
                ret.add(Gate::Buf(sigs[0]));
            }
            "NOT" => {
                ret.add(Gate::Buf(!sigs[0]));
            }
            "VDD" => {
                ret.add(Gate::Buf(Signal::one()));
            }
            "VSS" | "GND" => {
                ret.add(Gate::Buf(Signal::zero()));
            }
            "AND" => {
                ret.add(Gate::Nary(sigs, NaryType::And));
            }
            "NAND" => {
                ret.add(Gate::Nary(sigs, NaryType::Nand));
            }
            "OR" => {
                ret.add(Gate::Nary(sigs, NaryType::Or));
            }
            "NOR" => {
                ret.add(Gate::Nary(sigs, NaryType::Nor));
            }
            "XOR" => {
                ret.add(Gate::Nary(sigs, NaryType::Xor));
            }
            "XNOR" => {
                ret.add(Gate::Nary(sigs, NaryType::Xnor));
            }
            "MUX" => {
                ret.add(Gate::mux(sigs[0], sigs[1], sigs[2]));
            }
            "MAJ" => {
                ret.add(Gate::maj(sigs[0], sigs[1], sigs[2]));
            }
            _ => {
                if s[1].starts_with("LUT 0x") {
                    ret.add(Gate::lut(
                        sigs.as_ref(),
                        Lut::from_hex_string(sigs.len(), &s[1][6..]).unwrap(),
                    ));
                } else {
                    return Err(format!("Unknown gate type {}", s[1]));
                }
            }
        }
    }
    for o in outputs {
        ret.add_output(name_to_sig[o]);
    }
    ret.topo_sort();
    ret.check();
    Ok(ret)
}

/// Read a network in .bench format, as used by the ISCAS benchmarks
///
/// These files describe the design with simple statements like:
/// ```text
///     # This is a comment
///     INPUT(i0)
///     INPUT(i1)
///     x0 = AND(i0, i1)
///     x1 = NAND(x0, i1)
///     x2 = OR(x0, i0)
///     x3 = NOR(i0, x1)
///     x4 = XOR(x3, x2)
///     x5 = BUF(x4)
///     x6 = NOT(x5)
///     x7 = gnd
///     x8 = vdd
///     OUTPUT(x0)
/// ```
pub fn read_bench<R: Read>(r: R) -> Result<Network, String> {
    let mut statements = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for l in BufReader::new(r).lines() {
        if let Ok(s) = l {
            let t = s.trim().to_owned();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if !t.contains("=") {
                let parts: Vec<_> = t
                    .split(&['(', ')'])
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                assert_eq!(parts.len(), 2);
                if ["INPUT", "PINPUT"].contains(&parts[0]) {
                    inputs.push(parts[1].to_string());
                } else if ["OUTPUT", "POUTPUT"].contains(&parts[0]) {
                    outputs.push(parts[1].to_string());
                } else {
                    return Err(format!("Unknown keyword {}", parts[0]));
                }
            } else {
                let parts: Vec<_> = t
                    .split(&['=', '(', ',', ')'])
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                assert!(parts.len() >= 2);
                statements.push(parts);
            }
        } else {
            return Err("Error during file IO".to_string());
        }
    }
    network_from_statements(&statements, &inputs, &outputs)
}

/// Write a network in .bench format, as used by the ISCAS benchmarks
///
/// These files describe the design with simple statements like:
/// ```text
///     # This is a comment
///     INPUT(i0)
///     INPUT(i1)
///     x0 = AND(i0, i1)
///     x1 = NAND(x0, i1)
///     x2 = OR(x0, i0)
///     x3 = NOR(i0, x1)
///     x4 = XOR(x3, x2)
///     x5 = BUF(x4)
///     x6 = NOT(x5)
///     x7 = gnd
///     x8 = vdd
///     OUTPUT(x0)
/// ```
pub fn write_bench<W: Write>(w: &mut W, aig: &Network) {
    writeln!(w, "# .bench (ISCAS) file").unwrap();
    writeln!(w, "# Generated by quaigh").unwrap();
    for i in 0..aig.nb_inputs() {
        writeln!(w, "INPUT({})", aig.input(i)).unwrap();
    }
    writeln!(w).unwrap();
    for i in 0..aig.nb_outputs() {
        writeln!(w, "OUTPUT({})", sig_to_string(&aig.output(i))).unwrap();
    }
    writeln!(w).unwrap();
    for i in 0..aig.nb_nodes() {
        use Gate::*;
        let g = aig.gate(i);
        let rep = g
            .dependencies()
            .iter()
            .map(sig_to_string)
            .collect::<Vec<_>>()
            .join(", ");
        write!(w, "x{} = ", i).unwrap();
        match g {
            Binary(_, BinaryType::And) | Ternary(_, TernaryType::And) => {
                writeln!(w, "AND({})", rep).unwrap();
            }
            Binary(_, BinaryType::Xor) | Ternary(_, TernaryType::Xor) => {
                writeln!(w, "XOR({})", rep).unwrap();
            }
            Nary(_, tp) => match tp {
                NaryType::And => writeln!(w, "AND({})", rep).unwrap(),
                NaryType::Or => writeln!(w, "OR({})", rep).unwrap(),
                NaryType::Nand => writeln!(w, "NAND({})", rep).unwrap(),
                NaryType::Nor => writeln!(w, "NOR({})", rep).unwrap(),
                NaryType::Xor => writeln!(w, "XOR({})", rep).unwrap(),
                NaryType::Xnor => writeln!(w, "XNOR({})", rep).unwrap(),
            },
            Dff([d, en, res]) => {
                if *en != Signal::one() || *res != Signal::zero() {
                    writeln!(
                        w,
                        "DFFRSE({}, {}, gnd, {})",
                        sig_to_string(d),
                        sig_to_string(res),
                        sig_to_string(en)
                    )
                    .unwrap();
                } else {
                    writeln!(w, "DFF({})", sig_to_string(d)).unwrap();
                }
            }
            Ternary(_, TernaryType::Mux) => {
                writeln!(w, "MUX({})", rep).unwrap();
            }
            Ternary(_, TernaryType::Maj) => {
                writeln!(w, "MAJ({})", rep).unwrap();
            }
            Buf(s) => {
                if s.is_constant() {
                    writeln!(w, "{}", sig_to_string(s)).unwrap();
                } else if s.is_inverted() {
                    writeln!(w, "NOT({})", sig_to_string(&!s)).unwrap();
                } else {
                    writeln!(w, "BUF({})", rep).unwrap();
                }
            }
            Lut(lut) => {
                writeln!(w, "LUT 0x{}({})", lut.lut.to_hex_string(), rep).unwrap();
            }
        }
    }

    let signals_with_inv = get_inverted_signals(aig);
    for s in signals_with_inv {
        writeln!(w, "{}_n = NOT({})", s, s).unwrap();
    }
}

mod test {
    #[test]
    fn test_basic_readwrite() {
        use std::io::BufWriter;

        let example = "# .bench (ISCAS) file
# Generated by quaigh
INPUT(i0)
INPUT(i1)

OUTPUT(x0)
OUTPUT(x1)
OUTPUT(x2)
OUTPUT(x3)
OUTPUT(x4)
OUTPUT(x5)
OUTPUT(x6)

x0 = AND(i0, i1)
x1 = NAND(i0, i1)
x2 = OR(i0, i1)
x3 = NOR(i0, i1)
x4 = XOR(i0, i1)
x5 = BUF(i0)
x6 = NOT(i1)
x7 = NOT(x2)
x8 = gnd
x9 = vdd
x10 = XOR(  i0, i1 )
x11   =  gnd 
x12 = LUT 0x45fc (x0, x1, x2, x3)
";
        let aig = super::read_bench(example.as_bytes()).unwrap();
        assert_eq!(aig.nb_inputs(), 2);
        assert_eq!(aig.nb_outputs(), 7);
        assert_eq!(aig.nb_nodes(), 13);
        let mut buf = BufWriter::new(Vec::new());
        super::write_bench(&mut buf, &aig);
        String::from_utf8(buf.into_inner().unwrap()).unwrap();
    }
}
