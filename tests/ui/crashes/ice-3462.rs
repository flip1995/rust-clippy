#![warn(clippy::all)]
#![allow(clippy::blacklisted_name, clippy::equatable_if_let)]
#![allow(unused)]

/// Test for https://github.com/rust-lang/rust-clippy/issues/3462

enum Foo {
    Bar,
    Baz,
}

fn bar(foo: Foo) {
    macro_rules! baz {
        () => {
            if let Foo::Bar = foo {}
        };
    }

    baz!();
    baz!();
}

fn main() {}
