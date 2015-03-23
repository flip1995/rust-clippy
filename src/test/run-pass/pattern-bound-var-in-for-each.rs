// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests that trans_path checks whether a
// pattern-bound var is an upvar (when translating
// the for-each body)

// pretty-expanded FIXME #23616

fn foo(src: uint) {

    match Some(src) {
      Some(src_id) => {
        for _i in 0_usize..10_usize {
            let yyy = src_id;
            assert_eq!(yyy, 0_usize);
        }
      }
      _ => { }
    }
}

pub fn main() { foo(0_usize); }
