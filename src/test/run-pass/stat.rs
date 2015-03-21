// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fs::{File, TempDir};
use std::io::prelude::*;

pub fn main() {
    let dir = TempDir::new_in(".", "").unwrap();
    let path = dir.path().join("file");

    {
        match File::create(&path) {
            Err(..) => unreachable!(),
            Ok(f) => {
                let mut f = f;
                for _ in 0..1000 {
                    f.write(&[0]);
                }
            }
        }
    }

    assert!(path.exists());
    assert_eq!(path.metadata().unwrap().len(), 1000);
}
