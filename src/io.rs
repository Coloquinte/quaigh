//! Read and write Aigs to files

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
};

use crate::{Aig, Gate, Signal};

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
}

fn aig_from_statements(
    statements: &Vec<(String, GateType, Vec<String>)>,
    outputs: &Vec<String>,
) -> Aig {
    use GateType::*;

    // Check everything
    let generated_gates: HashSet<_> = statements.iter().map(|s| &s.0).collect();
    for (_, gate_type, deps) in statements {
        for dep in deps {
            assert!(
                generated_gates.contains(dep),
                "Gate input {dep} is not generated anywhere"
            );
        }
        match gate_type {
            Input => assert_eq!(deps.len(), 0),
            Dff | Buf | Not => assert_eq!(deps.len(), 1),
            _ => (),
        }
    }
    for output in outputs {
        assert!(
            generated_gates.contains(output),
            "Output {output} is not generated anywhere"
        );
    }

    // Compute a mapping between the two
    let mut ret = Aig::new();
    let mut name_to_sig = HashMap::new();
    let mut node_ind = 0u32;
    for (name, gate_type, _) in statements {
        match gate_type {
            Input => {
                // No node inserted
                name_to_sig.insert(name, ret.add_input());
            }
            Nand | Or => {
                // These require an inverted gate
                name_to_sig.insert(name, !Signal::from_var(node_ind));
                node_ind += 1;
            }
            _ => {
                name_to_sig.insert(name, Signal::from_var(node_ind));
                node_ind += 1;
            }
        }
    }

    // Setup the variables based on the mapping
    for (_, gate_type, deps) in statements {
        let sigs: Box<[Signal]> = deps.iter().map(|n| name_to_sig[n]).collect();
        let nsigs: Box<[Signal]> = deps.iter().map(|n| !name_to_sig[n]).collect();
        match gate_type {
            Input => (),
            Dff => {
                ret.add_raw_gate(Gate::Dff(sigs[0], Signal::one(), Signal::zero()));
            }
            Buf => {
                ret.add_raw_gate(Gate::And(sigs[0], sigs[0]));
            }
            Not => {
                ret.add_raw_gate(Gate::And(!sigs[0], !sigs[0]));
            }
            And | Nand => {
                ret.add_raw_gate(Gate::Andn(sigs));
            }
            Or | Nor => {
                ret.add_raw_gate(Gate::Andn(nsigs));
            }
            Xor => {
                ret.add_raw_gate(Gate::Xorn(sigs));
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
pub fn parse_bench<R: Read>(r: R) -> Result<Aig, String> {
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
                let g = match parts[1] {
                    "AND" => And,
                    "OR" => Or,
                    "NAND" => Nand,
                    "NOR" => Nor,
                    "XOR" => Xor,
                    "BUF" | "BUFF" => Buf,
                    "NOT" => Not,
                    "DFF" => Dff,
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

/// Open a file
pub fn parse_file(path: PathBuf) -> Aig {
    let ext = path.extension();
    match ext {
        None => panic!("No extension given"),
        Some(s) => {
            if s == "bench" {
                let f = File::open(path).unwrap();
                parse_bench(f).unwrap()
            } else {
                panic!("Unknown extension {}", s.to_string_lossy());
            }
        }
    }
}

/// Ad-hoc to_string function to represent signals in bench files
fn sig_to_string(s: &Signal) -> String {
    s.without_pol().to_string() + (if s.pol() { "_n" } else { "" })
}

/// Write a bench file, as used by the ISCAS benchmarks
///
/// These files describe the design with simple statements like:
/// ```text
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
pub fn write_bench<W: Write>(w: &mut W, aig: &Aig) {
    for i in 0..aig.nb_inputs() {
        writeln!(w, "INPUT({})", aig.input(i)).unwrap();
        writeln!(w, "i{}_n = NOT(i{})", i, i).unwrap();
    }
    writeln!(w).unwrap();
    for i in 0..aig.nb_outputs() {
        writeln!(w, "OUTPUT({})", aig.output(i)).unwrap();
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
            And(_, _) | And3(_, _, _) | Andn(_) => {
                writeln!(w, "AND({})", rep).unwrap();
            }
            Xor(_, _) | Xor3(_, _, _) | Xorn(_) => {
                writeln!(w, "XOR({})", rep).unwrap();
            }
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
        }
        writeln!(w, "x{}_n = NOT(x{})", i, i).unwrap();
    }
}

/// Write a file
pub fn write_file(path: PathBuf, aig: &Aig) {
    let ext = path.extension();
    match ext {
        None => panic!("No extension given"),
        Some(s) => {
            if s == "bench" {
                let mut f = File::create(path).unwrap();
                write_bench(&mut f, aig);
            } else {
                panic!("Unknown extension {}", s.to_string_lossy());
            }
        }
    }
}

mod test {

    #[test]
    fn test_simple_bench() {
        let example = "INPUT(i0)
INPUT(i1)
x0 = AND(i0, i1)
x1 = NAND(i0, i1)
x2 = OR(i0, i1)
x3 = NOR(i0, i1)
x4 = XOR(i0, i1)
x5 = BUF(i0)
x6 = NOT(i1)
OUTPUT(x0)
OUTPUT(x1)
OUTPUT(x2)
OUTPUT(x3)
OUTPUT(x4)
OUTPUT(x5)
OUTPUT(x6)";
        let aig = super::parse_bench(example.as_bytes()).unwrap();
        assert_eq!(aig.nb_inputs(), 2);
        assert_eq!(aig.nb_outputs(), 7);
        assert_eq!(aig.nb_nodes(), 7);
    }
}
