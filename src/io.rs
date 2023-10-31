use std::{io::{BufRead, BufReader, Read}, collections::HashSet};

use crate::Aig;

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

fn aig_from_statements(statements: &Vec<(String, GateType, Vec<String>)>, outputs: &Vec<String>) -> Aig {
    use GateType::*;

    // Check everything
    let generated_gates: HashSet<_> = statements.iter().map(|s| &s.0).collect();
    for (_, gate_type, deps) in statements {
        for dep in deps {
            assert!(generated_gates.contains(dep), "Gate input {dep} is not generated anywhere");
        }
        match gate_type {
            Input => assert_eq!(deps.len(), 0),
            Dff | Buf | Not => assert_eq!(deps.len(), 1),
            _ => (),
        }
    }
    for output in outputs {
        assert!(generated_gates.contains(output), "Output {output} is not generated anywhere");
    }

    // Perform a topological sort
    Aig::new()
}

/**
 * Parse a bench file, as used by the ISCAS benchmarks
 */
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
                    "BUF" => Buf,
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
    println!("Statements {:?}", statements);
    Ok(aig_from_statements(&statements, &outputs))
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
x6 = BUF(i1)
OUTPUT(x0)
OUTPUT(x1)
OUTPUT(x2)
OUTPUT(x3)
OUTPUT(x4)
OUTPUT(x5)
OUTPUT(x6)";
        super::parse_bench(example.as_bytes()).unwrap();
    }
}
