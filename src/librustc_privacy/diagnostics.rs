// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_snake_case)]

register_long_diagnostics! {

E0445: r##"
A private trait was used on a public type parameter bound. Erroneous code
examples:

```
trait Foo {
    fn dummy(&self) { }
}

pub trait Bar : Foo {} // error: private trait in exported type parameter bound
```

To solve this error, please ensure that the trait is also public and accessible
at the same level of the public functions or types which are bound on it.
Example:

```
pub trait Foo { // we set the Foo trait public
    fn dummy(&self) { }
}

pub trait Bar : Foo {} // ok!
```
"##,

E0446: r##"
A private type was used in an exported type signature. Erroneous code example:

```
mod Foo {
    struct Bar(u32);

    pub fn bar() -> Bar { // error: private type in exported type signature
        Bar(0)
    }
}
```

To solve this error, please ensure that the type is also public and accessible
at the same level of the public functions or types which use it. Example:

```
mod Foo {
    pub struct Bar(u32); // we set the Bar type public

    pub fn bar() -> Bar { // ok!
        Bar(0)
    }
}
```
"##,

E0447: r##"
The `pub` keyword was used inside a function. Erroneous code example:

```
fn foo() {
    pub struct Bar; // error: visibility has no effect inside functions
}
```

Since we cannot access inside function's elements, the visibility of its
elements does not impact outer code. So using the `pub` keyword in this context
is invalid.
"##,

}