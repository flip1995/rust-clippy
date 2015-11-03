// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(non_snake_case)]

fn main() {
    let a = 413;
    #[cfg(unset)]
    let a = ();
    assert_eq!(a, 413);

    let mut b = 612;
    #[cfg(unset)]
    {
        b = 1111;
    }
    assert_eq!(b, 612);

    #[cfg(unset)]
    undefined_fn();

    #[cfg(unset)]
    undefined_macro!();
    #[cfg(unset)]
    undefined_macro![];
    #[cfg(unset)]
    undefined_macro!{};

    // pretty printer bug...
    // #[cfg(unset)]
    // undefined_macro!{}

    let () = (#[cfg(unset)] 341,); // Should this also work on parens?
    let t = (1, #[cfg(unset)] 3, 4);
    assert_eq!(t, (1, 4));

    let f = |_: u32, _: u32| ();
    f(2, 1, #[cfg(unset)] 6);

    let _: u32 = a.clone(#[cfg(unset)] undefined);

    let _: [(); 0] = [#[cfg(unset)] 126];
    let t = [#[cfg(unset)] 1, 2, 6];
    assert_eq!(t, [2, 6]);

    {
        let r;
        #[cfg(unset)]
        (r = 5);
        #[cfg(not(unset))]
        (r = 10);
        assert_eq!(r, 10);
    }

    // check that macro expanded code works

    macro_rules! if_cfg {
        ($cfg:meta $ib:block else $eb:block) => {
            {
                let r;
                #[cfg($cfg)]
                (r = $ib);
                #[cfg(not($cfg))]
                (r = $eb);
                r
            }
        }
    }

    let n = if_cfg!(unset {
        413
    } else {
        612
    });

    assert_eq!((#[cfg(unset)] 1, #[cfg(not(unset))] 2), (2,));
    assert_eq!(n, 612);

    // check that lints work

    #[allow(non_snake_case)]
    let FOOBAR = {
        fn SYLADEX() {}
    };

    #[allow(non_snake_case)]
    {
        fn CRUXTRUDER() {}
    }
}
