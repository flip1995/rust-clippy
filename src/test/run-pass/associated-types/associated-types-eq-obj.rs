// run-pass
// Test equality constraints on associated types inside of an object type

// pretty-expanded FIXME #23616

pub trait Foo {
    type A;
    fn boo(&self) -> <Self as Foo>::A;
}

pub struct Bar;

impl Foo for char {
    type A = Bar;
    fn boo(&self) -> Bar { Bar }
}

fn baz(x: &Foo<A=Bar>) -> Bar {
    x.boo()
}

pub fn main() {
    let a = 'a';
    baz(&a);
}
