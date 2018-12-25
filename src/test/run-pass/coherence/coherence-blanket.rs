// run-pass
#![allow(unused_imports)]
// aux-build:coherence_lib.rs

// pretty-expanded FIXME #23616

extern crate coherence_lib as lib;
use lib::Remote1;

pub trait Local {
    fn foo(&self) { }
}

impl<T> Local for T { }

fn main() { }
