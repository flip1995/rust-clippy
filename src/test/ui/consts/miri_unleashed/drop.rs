// compile-flags: -Zunleash-the-miri-inside-of-you
// error-pattern: calling non-const function `<std::vec::Vec<i32> as std::ops::Drop>::drop`
#![deny(const_err)]

use std::mem::ManuallyDrop;

fn main() {}

static TEST_OK: () = {
    let v: Vec<i32> = Vec::new();
    let _v = ManuallyDrop::new(v);
};

// Make sure we catch executing bad drop functions.
// The actual error is tested by the error-pattern above.
static TEST_BAD: () = {
    let _v: Vec<i32> = Vec::new();
    //~^ WARN skipping const check
};
