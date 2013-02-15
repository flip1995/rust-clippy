// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// same as resource-cycle2, but be sure to give r multiple fields... 

// Don't leak the unique pointers

struct U {
    a: int,
    b: int,
    c: *int
}

struct R {
  v: U,
  w: int,
  x: *int,
}

impl Drop for R {
    fn finalize(&self) {
        unsafe {
            let _v2: ~int = cast::reinterpret_cast(&self.v.c);
            // let _v3: ~int = unsafe::reinterpret_cast(self.x);
        }
    }
}

fn r(v: U, w: int, _x: *int) -> R {
    unsafe {
        R {
            v: v,
            w: w,
            x: cast::reinterpret_cast(&0)
        }
    }
}

enum t = Node;

struct Node {
    mut next: Option<@t>,
    r: R
}

pub fn main() { 
    unsafe {
        let i1 = ~0xA;
        let i1p = cast::reinterpret_cast(&i1);
        cast::forget(i1);
        let i2 = ~0xA;
        let i2p = cast::reinterpret_cast(&i2);
        cast::forget(i2);

        let u1 = U {a: 0xB, b: 0xC, c: i1p};
        let u2 = U {a: 0xB, b: 0xC, c: i2p};

        let x1 = @t(Node{
            mut next: None,
            r: r(u1, 42, i1p)
        });
        let x2 = @t(Node{
            mut next: None,
            r: r(u2, 42, i2p)
        });
        x1.next = Some(x2);
        x2.next = Some(x1);
    }
}
