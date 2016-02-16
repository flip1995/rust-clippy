// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that the types of distinct fn items are not compatible by
// default. See also `run-pass/fn-item-type-*.rs`.

fn foo<T>(x: isize) -> isize { x * 2 }
fn bar<T>(x: isize) -> isize { x * 4 }

fn eq<T>(x: T, y: T) { }

trait Foo { fn foo() { /* this is a default fn */ } }
impl<T> Foo for T { /* `foo` is still default here */ }

fn main() {
    let f = if true { foo::<u8> } else { bar::<u8> };
    //~^ ERROR if and else have incompatible types
    //~| expected `fn(isize) -> isize {foo::<u8>}`
    //~| found `fn(isize) -> isize {bar::<u8>}`
    //~| expected fn item,
    //~| found a different fn item

    eq(foo::<u8>, bar::<u8>);
    //~^ ERROR mismatched types
    //~|  expected `fn(isize) -> isize {foo::<u8>}`
    //~|  found `fn(isize) -> isize {bar::<u8>}`
    //~|  expected fn item
    //~|  found a different fn item

    eq(foo::<u8>, foo::<i8>);
    //~^ ERROR mismatched types
    //~|  expected `fn(isize) -> isize {foo::<u8>}`
    //~|  found `fn(isize) -> isize {foo::<i8>}`

    eq(bar::<String>, bar::<Vec<u8>>);
    //~^ ERROR mismatched types
    //~|  expected `fn(isize) -> isize {bar::<collections::string::String>}`
    //~|  found `fn(isize) -> isize {bar::<collections::vec::Vec<u8>>}`
    //~|  expected struct `collections::string::String`
    //~|  found struct `collections::vec::Vec`

    // Make sure we distinguish between trait methods correctly.
    eq(<u8 as Foo>::foo, <u16 as Foo>::foo);
    //~^ ERROR mismatched types
    //~|  expected `fn() {Foo::foo}`
    //~|  found `fn() {Foo::foo}`
}
