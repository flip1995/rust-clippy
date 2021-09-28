// normalize-stderr-test "\(\d+ byte\)" -> "(N byte)"
// normalize-stderr-test "\(limit: \d+ byte\)" -> "(limit: N byte)"

#![deny(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::blacklisted_name, clippy::redundant_field_names)]

#[derive(Copy, Clone)]
struct Foo(u32);

#[derive(Copy, Clone)]
struct Bar([u8; 24]);

#[derive(Copy, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

struct FooRef<'a> {
    foo: &'a Foo,
}

type Baz = u32;

fn good(a: &mut u32, b: u32, c: &Bar) {}

fn good_return_implicit_lt_ref(foo: &Foo) -> &u32 {
    &foo.0
}

#[allow(clippy::needless_lifetimes)]
fn good_return_explicit_lt_ref<'a>(foo: &'a Foo) -> &'a u32 {
    &foo.0
}

fn good_return_implicit_lt_struct(foo: &Foo) -> FooRef {
    FooRef { foo }
}

#[allow(clippy::needless_lifetimes)]
fn good_return_explicit_lt_struct<'a>(foo: &'a Foo) -> FooRef<'a> {
    FooRef { foo }
}

fn bad(x: &u32, y: &Foo, z: &Baz) {}

impl Foo {
    fn good(self, a: &mut u32, b: u32, c: &Bar) {}

    fn good2(&mut self) {}

    fn bad(&self, x: &u32, y: &Foo, z: &Baz) {}

    fn bad2(x: &u32, y: &Foo, z: &Baz) {}

    fn bad_issue7518(self, other: &Self) {}
}

impl AsRef<u32> for Foo {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}

impl Bar {
    fn good(&self, a: &mut u32, b: u32, c: &Bar) {}

    fn bad2(x: &u32, y: &Foo, z: &Baz) {}
}

trait MyTrait {
    fn trait_method(&self, _foo: &Foo);
}

pub trait MyTrait2 {
    fn trait_method2(&self, _color: &Color);
}

impl MyTrait for Foo {
    fn trait_method(&self, _foo: &Foo) {
        unimplemented!()
    }
}

#[allow(unused_variables)]
mod issue3992 {
    pub trait A {
        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn a(b: &u16) {}
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn c(d: &u16) {}
}

mod issue5876 {
    // Don't lint here as it is always inlined
    #[inline(always)]
    fn foo_always(x: &i32) {
        println!("{}", x);
    }

    #[inline(never)]
    fn foo_never(x: &i32) {
        println!("{}", x);
    }

    #[inline]
    fn foo(x: &i32) {
        println!("{}", x);
    }
}

fn main() {
    let (mut foo, bar) = (Foo(0), Bar([0; 24]));
    let (mut a, b, c, x, y, z) = (0, 0, Bar([0; 24]), 0, Foo(0), 0);
    good(&mut a, b, &c);
    good_return_implicit_lt_ref(&y);
    good_return_explicit_lt_ref(&y);
    bad(&x, &y, &z);
    foo.good(&mut a, b, &c);
    foo.good2();
    foo.bad(&x, &y, &z);
    Foo::bad2(&x, &y, &z);
    bar.good(&mut a, b, &c);
    Bar::bad2(&x, &y, &z);
    foo.as_ref();
}
