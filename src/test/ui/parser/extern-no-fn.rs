// compile-flags: -Z parse-only

extern { //~ ERROR missing `fn`, `type`, or `static` for extern-item declaration
    f();
}

fn main() {
}
