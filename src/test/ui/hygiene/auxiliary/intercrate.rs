// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(decl_macro)]

pub mod foo {
    pub use self::bar::m;
    mod bar {
        fn f() -> u32 { 1 }
        pub macro m() {
            f();
        }
    }
}

pub struct SomeType;

pub macro uses_dollar_crate() {
    type Alias = $crate::SomeType;
}
