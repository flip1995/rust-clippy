struct Foo {
    a: isize,
    b: isize,
}

struct Bar {
    a: isize,
    b: usize,
}

fn want_foo(f: Foo) {}
fn have_bar(b: Bar) {
    want_foo(b); //~  ERROR mismatched types
                 //~| expected struct `Foo`, found struct `Bar`
}

fn main() {}
