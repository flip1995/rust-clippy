// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_type = "rustc-macro"]
#![allow(warnings)]

pub fn a() {} //~ ERROR: cannot export any items
pub struct B; //~ ERROR: cannot export any items
pub enum C {} //~ ERROR: cannot export any items
pub mod d {} //~ ERROR: cannot export any items

mod e {}
struct F;
enum G {}
fn h() {}
