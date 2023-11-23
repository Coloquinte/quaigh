//! Read and write Aigs to files

mod bench;
mod patterns;

use std::fs::File;
use std::path::PathBuf;

pub use bench::read_bench;
pub use bench::write_bench;
pub use patterns::write_patterns;

use crate::Aig;

/// Parse a logic network from a file
///
/// Following extensions are supported: .bench
pub fn parse_file(path: PathBuf) -> Aig {
    let ext = path.extension();
    match ext {
        None => panic!("No extension given"),
        Some(s) => {
            if s == "bench" {
                let f = File::open(path).unwrap();
                read_bench(f).unwrap()
            } else {
                panic!("Unknown extension {}", s.to_string_lossy());
            }
        }
    }
}

/// Write a logic network to a file
///
/// Following extensions are supported: .bench
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
