// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// pretty-expanded FIXME #23616

fn get<T>(opt: &Option<T>) -> &T {
    match *opt {
      Some(ref v) => v,
      None => panic!("none")
    }
}

pub fn main() {
    let mut x = Some(23);

    {
        let y = get(&x);
        assert_eq!(*y, 23);
    }

    x = Some(24);

    {
        let y = get(&x);
        assert_eq!(*y, 24);
    }
}
