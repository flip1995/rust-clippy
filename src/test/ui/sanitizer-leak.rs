// needs-sanitizer-support
// only-x86_64
//
// compile-flags: -Z sanitizer=leak -O
//
// run-fail
// error-pattern: LeakSanitizer: detected memory leaks

#![feature(test)]

use std::hint::black_box;
use std::mem;

fn main() {
    for _ in 0..10 {
        let xs = vec![1, 2, 3];
        // Prevent compiler from removing the memory allocation.
        let xs = black_box(xs);
        mem::forget(xs);
    }
}
