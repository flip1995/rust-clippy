fn id<T>(x: T) -> T { x }

fn f(_x: &isize) -> &isize {
    return &id(3); //~ ERROR borrowed value does not live long enough
}

fn main() {
}
