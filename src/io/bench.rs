//! IO for .bench (ISCAS) files

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};

use crate::{Gate, NaryType, Network, Signal};

#[derive(Clone, Debug)]
enum GateType {
    Input,
    Dff,
    Buf,
    Not,
    And,
    Or,
    Nand,
    Nor,
    Xor,
    Xnor,
    Vdd,
    Vss,
}

fn aig_from_statements(
    statements: &Vec<(String, GateType, Vec<String>)>,
    outputs: &Vec<String>,
) -> Network {
    use GateType::*;

    // Compute a mapping between the two
    let mut ret = Network::new();
    let mut name_to_sig = HashMap::new();
    let mut node_ind = 0u32;
    for (name, gate_type, _) in statements {
        match gate_type {
            Input => {
                // No node inserted
                name_to_sig.insert(name.clone(), ret.add_input());
            }
            _ => {
                name_to_sig.insert(name.clone(), Signal::from_var(node_ind));
                node_ind += 1;
            }
        }
    }

    // ABC-style naming for constant signals
    if !name_to_sig.contains_key("vdd") {
        name_to_sig.insert("vdd".to_string(), Signal::one());
    }
    if !name_to_sig.contains_key("gnd") {
        name_to_sig.insert("gnd".to_string(), Signal::zero());
    }

    // Check everything
    for (_, gate_type, deps) in statements {
        for dep in deps {
            assert!(
                name_to_sig.contains_key(dep),
                "Gate input {dep} is not generated anywhere"
            );
        }
        match gate_type {
            Input => assert_eq!(deps.len(), 0),
            Dff | Buf | Not => assert_eq!(deps.len(), 1),
            Vdd | Vss => assert_eq!(deps.len(), 0),
            _ => (),
        }
    }
    for output in outputs {
        assert!(
            name_to_sig.contains_key(output),
            "Output {output} is not generated anywhere"
        );
    }

    // Setup the variables based on the mapping
    for (_, gate_type, deps) in statements {
        let sigs: Box<[Signal]> = deps.iter().map(|n| name_to_sig[n]).collect();
        match gate_type {
            Input => (),
            Dff => {
                ret.add(Gate::Dff(sigs[0], Signal::one(), Signal::zero()));
            }
            Buf => {
                ret.add(Gate::Buf(sigs[0]));
            }
            Not => {
                ret.add(Gate::Buf(!sigs[0]));
            }
            Vdd => {
                ret.add(Gate::Buf(Signal::one()));
            }
            Vss => {
                ret.add(Gate::Buf(Signal::zero()));
            }
            And => {
                ret.add(Gate::Nary(sigs, NaryType::And));
            }
            Nand => {
                ret.add(Gate::Nary(sigs, NaryType::Nand));
            }
            Or => {
                ret.add(Gate::Nary(sigs, NaryType::Or));
            }
            Nor => {
                ret.add(Gate::Nary(sigs, NaryType::Nor));
            }
            Xor => {
                ret.add(Gate::Nary(sigs, NaryType::Xor));
            }
            Xnor => {
                ret.add(Gate::Nary(sigs, NaryType::Xnor));
            }
        }
    }
    for o in outputs {
        ret.add_output(name_to_sig[o]);
    }
    ret.topo_sort();
    ret
}

/// Parse a bench file, as used by the ISCAS benchmarks
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
///     OUTPUT(x0)
/// ```
pub fn read_bench<R: Read>(r: R) -> Result<Network, String> {
    use GateType::*;

    let mut statements = Vec::new();
    let mut outputs = Vec::new();
    for l in BufReader::new(r).lines() {
        if let Ok(s) = l {
            let t = s.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let parts: Vec<_> = t
                .split(&['=', '(', ',', ')'])
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if parts.len() == 2 && ["INPUT", "OUTPUT"].contains(&parts[0]) {
                if parts[0] == "INPUT" {
                    statements.push((parts[1].to_string(), Input, Vec::new()));
                } else {
                    outputs.push(parts[1].to_string());
                }
            } else if parts.len() < 2 {
                panic!("Too few items on the line");
            } else {
                let inputs: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
                let gate = parts[0].to_string();
                let g = match parts[1].to_uppercase().as_str() {
                    "AND" => And,
                    "OR" => Or,
                    "NAND" => Nand,
                    "NOR" => Nor,
                    "XOR" => Xor,
                    "XNOR" => Xnor,
                    "BUF" | "BUFF" => Buf,
                    "NOT" => Not,
                    "DFF" => Dff,
                    "VDD" => Vdd,
                    "GND" => Vss,
                    _ => panic!("Unwnown gate type {}", parts[1]),
                };
                statements.push((gate, g, inputs));
            }
        } else {
            return Err("Error during file IO".to_string());
        }
    }
    Ok(aig_from_statements(&statements, &outputs))
}

/// Ad-hoc to_string function to represent signals in bench files
fn sig_to_string(s: &Signal) -> String {
    if *s == Signal::one() {
        return "vdd".to_string();
    }
    if *s == Signal::zero() {
        return "gnd".to_string();
    }
    s.without_inversion().to_string() + (if s.is_inverted() { "_n" } else { "" })
}

/// Write a bench file, as used by the ISCAS benchmarks
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
            And(_, _) | And3(_, _, _) => {
                writeln!(w, "AND({})", rep).unwrap();
            }
            Xor(_, _) | Xor3(_, _, _) => {
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
            Dff(d, en, res) => {
                if *en != Signal::one() || *res != Signal::zero() {
                    panic!("Only DFF without enable or reset are supported");
                }
                writeln!(w, "DFF({})", sig_to_string(d)).unwrap();
            }
            Mux(_, _, _) => {
                writeln!(w, "MUX({})", rep).unwrap();
            }
            Maj(_, _, _) => {
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
        }
    }

    // Generate signals where the inversion is required
    let mut signals_with_inv = HashSet::new();
    for o in 0..aig.nb_outputs() {
        let s = aig.output(o);
        if s.is_inverted() && !s.is_constant() {
            signals_with_inv.insert(!s);
        }
    }
    for i in 0..aig.nb_nodes() {
        if matches!(aig.gate(i), Gate::Buf(_)) {
            // Buf(!x) is exported directly as a Not
            continue;
        }
        for s in aig.gate(i).dependencies() {
            if s.is_inverted() && !s.is_constant() {
                signals_with_inv.insert(!s);
            }
        }
    }
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
";
        let aig = super::read_bench(example.as_bytes()).unwrap();
        assert_eq!(aig.nb_inputs(), 2);
        assert_eq!(aig.nb_outputs(), 7);
        assert_eq!(aig.nb_nodes(), 10);
        let mut buf = BufWriter::new(Vec::new());
        super::write_bench(&mut buf, &aig);
        String::from_utf8(buf.into_inner().unwrap()).unwrap();
    }
}
