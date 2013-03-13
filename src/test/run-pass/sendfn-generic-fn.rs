// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// xfail-fast

pub fn main() { test05(); }

struct Pair<A,B> { a: A, b: B }

fn make_generic_record<A:Copy,B:Copy>(a: A, b: B) -> Pair<A,B> {
    return Pair {a: a, b: b};
}

fn test05_start(f: &~fn(v: float, v: ~str) -> Pair<float, ~str>) {
    let p = (*f)(22.22f, ~"Hi");
    debug!(copy p);
    fail_unless!(p.a == 22.22f);
    fail_unless!(p.b == ~"Hi");

    let q = (*f)(44.44f, ~"Ho");
    debug!(copy q);
    fail_unless!(q.a == 44.44f);
    fail_unless!(q.b == ~"Ho");
}

fn spawn<A:Copy,B:Copy>(f: extern fn(&~fn(A,B)->Pair<A,B>)) {
    let arg: ~fn(A, B) -> Pair<A,B> = |a, b| make_generic_record(a, b);
    task::spawn(|| f(&arg));
}

fn test05() {
    spawn::<float,~str>(test05_start);
}
