// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-emscripten
// compile-pass
// skip-codegen
#![feature(asm)]

macro_rules! interrupt_handler {
    () => {
        unsafe fn _interrupt_handler() {
            asm!("pop  eax" :::: "intel");
        }
    }
}
interrupt_handler!{}


fn main() {
}
