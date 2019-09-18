// compile-fail

#![feature(specialization)]

// Test that attempting to override a non-default method or one not in the
// parent impl causes an error

trait Foo {
    fn foo(&self) -> bool { true }
}

// Specialization tree for Foo:
//
//       Box<T>              Vec<T>
//        / \                 / \
// Box<i32>  Box<i64>   Vec<()>  Vec<bool>

impl<T> Foo for Box<T> {
    fn foo(&self) -> bool { false }
}

// Allowed
impl Foo for Box<i32> {}

// Can't override a non-`default` fn
impl Foo for Box<i64> {
    fn foo(&self) -> bool { true }
    //~^ error: `foo` specializes an item from a parent `impl`, but that item is not marked `default`
}


// Doesn't mention the method = provided body is used and the method is final
impl<T> Foo for Vec<T> {}

// Allowed
impl Foo for Vec<()> {}

impl Foo for Vec<bool> {
    fn foo(&self) -> bool { true }
    //~^ error: `foo` specializes an item from a parent `impl`, but that item is not marked `default`
}

fn main() {}
