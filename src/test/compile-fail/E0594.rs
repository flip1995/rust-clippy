// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// revisions: ast mir
//[mir]compile-flags: -Z emit-end-regions -Z borrowck-mir

static NUM: i32 = 18;

fn main() {
    NUM = 20; //[ast]~ ERROR E0594
              //[mir]~^ ERROR cannot assign to immutable static item (Ast)
              //[mir]~| ERROR cannot assign to immutable static item `NUM` (Mir)
}
