// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:namespaced_enums.rs
#![feature(globs)]

extern crate namespaced_enums;

mod m {
    pub use namespaced_enums::Foo::*;
}

pub fn main() {
    use namespaced_enums::Foo::*;

    foo(); //~ ERROR unresolved name `foo`
    m::foo(); //~ ERROR unresolved name `m::foo`
    bar(); //~ ERROR unresolved name `bar`
    m::bar(); //~ ERROR unresolved name `m::bar`
}

