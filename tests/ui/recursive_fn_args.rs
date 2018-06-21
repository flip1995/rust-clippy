


fn ok_rec(a: bool) {
    if a {
        ok_rec(!a);
    }
}

fn bad_rec(a: bool) {
    if 42 == 0 {
        bad_rec(a);
    }
}

fn bad_rec_mul_args(a: i32, mut b: u8, c: bool, d: Foo) {
    if a < 10 && c {
        b = 5;
        bad_rec_mul_args(a, b, c, d);
    }
}

fn ok_rec_switch(a: bool, b: bool) {
    if b {
        ok_rec_switch(b, a);
    }
}

fn ok_method_call(a: &mut Foo) {
    a.mut_foo();
    if false {
        ok_method_call(a);
    }
}

fn ok_mut_fn(a: bool) {
    if not(a) {
        ok_mut_fn(a);
    }
}

fn not(x: bool) -> bool {
    !x
}

fn main() {}

struct Foo(u32);

impl Foo {
    fn mut_foo(&mut self) {
        self.0 += 1;
    }
}
