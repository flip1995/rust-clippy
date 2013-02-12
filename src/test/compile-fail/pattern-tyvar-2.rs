// -*- rust -*-
// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


extern mod std;
use option::Some;

// error-pattern: mismatched types

enum bar { t1((), Option<~[int]>), t2, }

fn foo(t: bar) -> int { match t { t1(_, Some(x)) => { return x * 3; } _ => { fail!(); } } }

fn main() { }
