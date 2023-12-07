//! Read and write logic networks to files

mod bench;
mod patterns;

use std::fs::File;
use std::path::PathBuf;

pub use bench::{read_bench, write_bench};
pub use patterns::{read_patterns, write_patterns};

use crate::Network;

/// Read a logic network from a file
///
/// Following extensions are supported: .bench
pub fn read_network_file(path: &PathBuf) -> Network {
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
pub fn write_network_file(path: &PathBuf, aig: &Network) {
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

/// Read patterns from a file
pub fn read_pattern_file(path: &PathBuf) -> Vec<Vec<Vec<bool>>> {
    let f = File::open(path).unwrap();
    read_patterns(f).unwrap()
}

/// Write patterns to a file
pub fn write_pattern_file(path: &PathBuf, patterns: &Vec<Vec<Vec<bool>>>) {
    let mut f = File::create(path).unwrap();
    write_patterns(&mut f, patterns);
}
