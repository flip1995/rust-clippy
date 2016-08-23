// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// no-prefer-dynamic

#![crate_type = "rustc-macro"]
#![feature(rustc_macro)]
#![feature(rustc_macro_lib)]

extern crate rustc_macro;

use rustc_macro::TokenStream;

#[rustc_macro_derive(AddImpl)]
// #[cfg(rustc_macro)]
pub fn derive(input: TokenStream) -> TokenStream {
    (input.to_string() + "
        impl B {
            fn foo(&self) {}
        }

        fn foo() {}

        mod bar { pub fn foo() {} }
    ").parse().unwrap()
}
