// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that the same coverage rules apply even if the local type appears in the
// list of type parameters, not the self type.

// aux-build:coherence-lib.rs

extern crate "coherence-lib" as lib;
use lib::{Remote1, Pair};

pub struct Local<T>(T);

impl<T,U> Remote1<Pair<T,Local<U>>> for i32 { }
//~^ ERROR type parameter `T` is not constrained

fn main() { }
