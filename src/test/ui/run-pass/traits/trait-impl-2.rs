// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
#![allow(non_snake_case)]

// pretty-expanded FIXME #23616

pub mod Foo {
    pub trait Trait {
        fn foo(&self);
    }
}

mod Bar {
    impl<'a> ::Foo::Trait+'a {
        fn bar(&self) { self.foo() }
    }
}

fn main() {}
