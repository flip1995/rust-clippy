//@aux-build:proc_macros.rs

#![warn(clippy::needless_lifetimes, clippy::elidable_lifetime_names)]
#![allow(
    unused,
    clippy::boxed_local,
    clippy::extra_unused_type_parameters,
    clippy::needless_pass_by_value,
    clippy::redundant_allocation,
    clippy::unnecessary_wraps,
    dyn_drop,
    clippy::get_first,
    elided_named_lifetimes
)]

extern crate proc_macros;
use proc_macros::inline_macros;

fn distinct_lifetimes<'a, 'b>(_x: &'a u8, _y: &'b u8, _z: u8) {}
//~^ needless_lifetimes

fn distinct_and_static<'a, 'b>(_x: &'a u8, _y: &'b u8, _z: &'static u8) {}
//~^ needless_lifetimes

// No error; same lifetime on two params.
fn same_lifetime_on_input<'a>(_x: &'a u8, _y: &'a u8) {}

// No error; static involved.
fn only_static_on_input(_x: &u8, _y: &u8, _z: &'static u8) {}

fn mut_and_static_input(_x: &mut u8, _y: &'static str) {}

fn in_and_out<'a>(x: &'a u8, _y: u8) -> &'a u8 {
    //~^ needless_lifetimes
    x
}

// No error; multiple input refs.
fn multiple_in_and_out_1<'a>(x: &'a u8, _y: &'a u8) -> &'a u8 {
    x
}

// Error; multiple input refs, but the output lifetime is not elided, i.e., the following is valid:
//   fn multiple_in_and_out_2a<'a>(x: &'a u8, _y: &u8) -> &'a u8
//                                                ^^^
fn multiple_in_and_out_2a<'a, 'b>(x: &'a u8, _y: &'b u8) -> &'a u8 {
    //~^ needless_lifetimes
    x
}

// Error; multiple input refs, but the output lifetime is not elided, i.e., the following is valid:
//   fn multiple_in_and_out_2b<'b>(_x: &u8, y: &'b u8) -> &'b u8
//                                     ^^^
fn multiple_in_and_out_2b<'a, 'b>(_x: &'a u8, y: &'b u8) -> &'b u8 {
    //~^ needless_lifetimes
    y
}

// No error; multiple input refs
async fn func<'a>(args: &[&'a str]) -> Option<&'a str> {
    args.get(0).cloned()
}

// No error; static involved.
fn in_static_and_out<'a>(x: &'a u8, _y: &'static u8) -> &'a u8 {
    x
}

// Error; multiple input refs, but the output lifetime is not elided, i.e., the following is valid:
//   fn deep_reference_1a<'a>(x: &'a u8, _y: &u8) -> Result<&'a u8, ()>
//                                           ^^^
fn deep_reference_1a<'a, 'b>(x: &'a u8, _y: &'b u8) -> Result<&'a u8, ()> {
    //~^ needless_lifetimes
    Ok(x)
}

// Error; multiple input refs, but the output lifetime is not elided, i.e., the following is valid:
//   fn deep_reference_1b<'b>(_x: &u8, y: &'b u8) -> Result<&'b u8, ()>
//                                ^^^
fn deep_reference_1b<'a, 'b>(_x: &'a u8, y: &'b u8) -> Result<&'b u8, ()> {
    //~^ needless_lifetimes
    Ok(y)
}

// No error; two input refs.
fn deep_reference_2<'a>(x: Result<&'a u8, &'a u8>) -> &'a u8 {
    x.unwrap()
}

fn deep_reference_3<'a>(x: &'a u8, _y: u8) -> Result<&'a u8, ()> {
    //~^ needless_lifetimes
    Ok(x)
}

// Where-clause, but without lifetimes.
fn where_clause_without_lt<'a, T>(x: &'a u8, _y: u8) -> Result<&'a u8, ()>
//~^ needless_lifetimes
where
    T: Copy,
{
    Ok(x)
}

// No error; see below.
fn fn_bound_3<'a, F: FnOnce(&'a i32)>(x: &'a i32, f: F) {
    f(x);
}

fn fn_bound_3_cannot_elide() {
    let x = 42;
    let p = &x;
    let mut q = &x;
    // This will fail if we elide lifetimes of `fn_bound_3`.
    fn_bound_3(p, |y| q = y);
}

// No error; multiple input refs.
fn fn_bound_4<'a, F: FnOnce() -> &'a ()>(cond: bool, x: &'a (), f: F) -> &'a () {
    if cond { x } else { f() }
}

struct X {
    x: u8,
}

impl X {
    fn self_and_out<'s>(&'s self) -> &'s u8 {
        //~^ needless_lifetimes
        &self.x
    }

    // Error; multiple input refs, but the output lifetime is not elided, i.e., the following is valid:
    //   fn self_and_in_out_1<'s>(&'s self, _x: &u8) -> &'s u8
    //                                          ^^^
    fn self_and_in_out_1<'s, 't>(&'s self, _x: &'t u8) -> &'s u8 {
        //~^ needless_lifetimes
        &self.x
    }

    // Error; multiple input refs, but the output lifetime is not elided, i.e., the following is valid:
    //   fn self_and_in_out_2<'t>(&self, x: &'t u8) -> &'t u8
    //                            ^^^^^
    fn self_and_in_out_2<'s, 't>(&'s self, x: &'t u8) -> &'t u8 {
        //~^ needless_lifetimes
        x
    }

    fn distinct_self_and_in<'s, 't>(&'s self, _x: &'t u8) {}
    //~^ needless_lifetimes

    // No error; same lifetimes on two params.
    fn self_and_same_in<'s>(&'s self, _x: &'s u8) {}
}

struct Foo<'a>(&'a u8);

impl<'a> Foo<'a> {
    // No error; lifetime `'a` not defined in method.
    fn self_shared_lifetime(&self, _: &'a u8) {}
    // No error; bounds exist.
    fn self_bound_lifetime<'b: 'a>(&self, _: &'b u8) {}
}

fn already_elided<'a>(_: &u8, _: &'a u8) -> &'a u8 {
    unimplemented!()
}

// Warning; two input lifetimes, but the output lifetime is not elided, i.e., the following is
// valid:
//   fn struct_with_lt4b<'b>(_foo: &Foo<'b>) -> &'b str
//                                 ^^^^
fn struct_with_lt4b<'a, 'b>(_foo: &'a Foo<'b>) -> &'b str {
    //~^ needless_lifetimes
    unimplemented!()
}

trait WithLifetime<'a> {}

type WithLifetimeAlias<'a> = dyn WithLifetime<'a>;

// Should not warn because it won't build without the lifetime.
fn trait_obj_elided<'a>(_arg: &'a dyn WithLifetime) -> &'a str {
    unimplemented!()
}

// Should warn because there is no lifetime on `Drop`, so this would be
// unambiguous if we elided the lifetime.
fn trait_obj_elided2<'a>(_arg: &'a dyn Drop) -> &'a str {
    //~^ needless_lifetimes
    unimplemented!()
}

type FooAlias<'a> = Foo<'a>;

// Warning; two input lifetimes, but the output lifetime is not elided, i.e., the following is
// valid:
//   fn alias_with_lt4b<'b>(_foo: &FooAlias<'b>) -> &'b str
//                                ^^^^^^^^^
fn alias_with_lt4b<'a, 'b>(_foo: &'a FooAlias<'b>) -> &'b str {
    //~^ needless_lifetimes
    unimplemented!()
}

fn named_input_elided_output<'a>(_arg: &'a str) -> &str {
    //~^ needless_lifetimes
    unimplemented!()
}

fn elided_input_named_output<'a>(_arg: &str) -> &'a str {
    unimplemented!()
}

fn trait_bound_ok<'a, T: WithLifetime<'static>>(_: &'a u8, _: T) {
    //~^ needless_lifetimes
    unimplemented!()
}
fn trait_bound<'a, T: WithLifetime<'a>>(_: &'a u8, _: T) {
    unimplemented!()
}

// Don't warn on these; see issue #292.
fn trait_bound_bug<'a, T: WithLifetime<'a>>() {
    unimplemented!()
}

// See issue #740.
struct Test {
    vec: Vec<usize>,
}

impl Test {
    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = usize> + 'a> {
        unimplemented!()
    }
}

trait LintContext<'a> {}

fn f<'a, T: LintContext<'a>>(_: &T) {}

fn test<'a>(x: &'a [u8]) -> u8 {
    let y: &'a u8 = &x[5];
    *y
}

// Make sure we still warn on implementations
mod issue4291 {
    trait BadTrait {
        fn needless_lt<'a>(x: &'a u8) {}
        //~^ needless_lifetimes
    }

    impl BadTrait for () {
        fn needless_lt<'a>(_x: &'a u8) {}
        //~^ needless_lifetimes
    }
}

mod nested_elision_sites {
    // issue #issue2944

    // closure trait bounds subject to nested elision
    // don't lint because they refer to outer lifetimes
    fn trait_fn<'a>(i: &'a i32) -> impl Fn() -> &'a i32 {
        move || i
    }
    fn trait_fn_mut<'a>(i: &'a i32) -> impl FnMut() -> &'a i32 {
        move || i
    }
    fn trait_fn_once<'a>(i: &'a i32) -> impl FnOnce() -> &'a i32 {
        move || i
    }

    // don't lint
    fn impl_trait_in_input_position<'a>(f: impl Fn() -> &'a i32) -> &'a i32 {
        f()
    }
    fn impl_trait_in_output_position<'a>(i: &'a i32) -> impl Fn() -> &'a i32 {
        move || i
    }
    // lint
    fn impl_trait_elidable_nested_named_lifetimes<'a>(i: &'a i32, f: impl for<'b> Fn(&'b i32) -> &'b i32) -> &'a i32 {
        f(i)
    }
    fn impl_trait_elidable_nested_anonymous_lifetimes<'a>(i: &'a i32, f: impl Fn(&i32) -> &i32) -> &'a i32 {
        //~^ needless_lifetimes
        f(i)
    }

    // don't lint
    fn generics_not_elidable<'a, T: Fn() -> &'a i32>(f: T) -> &'a i32 {
        f()
    }
    // lint
    fn generics_elidable<'a, T: Fn(&i32) -> &i32>(i: &'a i32, f: T) -> &'a i32 {
        //~^ needless_lifetimes
        f(i)
    }

    // don't lint
    fn where_clause_not_elidable<'a, T>(f: T) -> &'a i32
    where
        T: Fn() -> &'a i32,
    {
        f()
    }
    // lint
    fn where_clause_elidable<'a, T>(i: &'a i32, f: T) -> &'a i32
    //~^ needless_lifetimes
    where
        T: Fn(&i32) -> &i32,
    {
        f(i)
    }

    // don't lint
    fn pointer_fn_in_input_position<'a>(f: fn(&'a i32) -> &'a i32, i: &'a i32) -> &'a i32 {
        f(i)
    }
    fn pointer_fn_in_output_position<'a>(_: &'a i32) -> fn(&'a i32) -> &'a i32 {
        |i| i
    }
    // lint
    fn pointer_fn_elidable<'a>(i: &'a i32, f: fn(&i32) -> &i32) -> &'a i32 {
        //~^ needless_lifetimes
        f(i)
    }

    // don't lint
    fn nested_fn_pointer_1<'a>(_: &'a i32) -> fn(fn(&'a i32) -> &'a i32) -> i32 {
        |f| 42
    }
    fn nested_fn_pointer_2<'a>(_: &'a i32) -> impl Fn(fn(&'a i32)) {
        |f| ()
    }

    // lint
    fn nested_fn_pointer_3<'a>(_: &'a i32) -> fn(fn(&i32) -> &i32) -> i32 {
        //~^ needless_lifetimes
        |f| 42
    }
    fn nested_fn_pointer_4<'a>(_: &'a i32) -> impl Fn(fn(&i32)) {
        //~^ needless_lifetimes
        |f| ()
    }
}

mod issue6159 {
    use std::ops::Deref;
    pub fn apply_deref<'a, T, F, R>(x: &'a T, f: F) -> R
    where
        T: Deref,
        F: FnOnce(&'a T::Target) -> R,
    {
        f(x.deref())
    }
}

mod issue7296 {
    use std::rc::Rc;
    use std::sync::Arc;

    struct Foo;
    impl Foo {
        fn implicit<'a>(&'a self) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
        fn implicit_mut<'a>(&'a mut self) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
        #[clippy::msrv = "1.81"]
        fn explicit<'a>(self: &'a Arc<Self>) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
        #[clippy::msrv = "1.81"]
        fn explicit_mut<'a>(self: &'a mut Rc<Self>) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
        #[clippy::msrv = "1.80"]
        fn explicit_older<'a>(self: &'a Arc<Self>) -> &'a () {
            &()
        }
        #[clippy::msrv = "1.80"]
        fn explicit_mut_older<'a>(self: &'a mut Rc<Self>) -> &'a () {
            &()
        }

        fn lifetime_elsewhere<'a>(self: Box<Self>, here: &'a ()) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
    }

    trait Bar {
        fn implicit<'a>(&'a self) -> &'a ();
        //~^ needless_lifetimes
        fn implicit_provided<'a>(&'a self) -> &'a () {
            //~^ needless_lifetimes
            &()
        }

        #[clippy::msrv = "1.81"]
        fn explicit<'a>(self: &'a Arc<Self>) -> &'a ();
        //~^ needless_lifetimes
        #[clippy::msrv = "1.81"]
        fn explicit_provided<'a>(self: &'a Arc<Self>) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
        #[clippy::msrv = "1.80"]
        fn explicit_older<'a>(self: &'a Arc<Self>) -> &'a ();
        #[clippy::msrv = "1.80"]
        fn explicit_provided_older<'a>(self: &'a Arc<Self>) -> &'a () {
            &()
        }

        fn lifetime_elsewhere<'a>(self: Box<Self>, here: &'a ()) -> &'a ();
        //~^ needless_lifetimes
        fn lifetime_elsewhere_provided<'a>(self: Box<Self>, here: &'a ()) -> &'a () {
            //~^ needless_lifetimes
            &()
        }
    }
}

mod pr_9743_false_negative_fix {
    #![allow(unused)]

    fn foo<'a>(x: &'a u8, y: &'_ u8) {}
    //~^ needless_lifetimes

    fn bar<'a>(x: &'a u8, y: &'_ u8, z: &'_ u8) {}
    //~^ needless_lifetimes
}

mod pr_9743_output_lifetime_checks {
    #![allow(unused)]

    // lint: only one input
    fn one_input<'a>(x: &'a u8) -> &'a u8 {
        //~^ needless_lifetimes
        unimplemented!()
    }

    // lint: multiple inputs, output would not be elided
    fn multiple_inputs_output_not_elided<'a, 'b>(x: &'a u8, y: &'b u8, z: &'b u8) -> &'b u8 {
        //~^ needless_lifetimes
        unimplemented!()
    }

    // don't lint: multiple inputs, output would be elided (which would create an ambiguity)
    fn multiple_inputs_output_would_be_elided<'a, 'b>(x: &'a u8, y: &'b u8, z: &'b u8) -> &'a u8 {
        unimplemented!()
    }
}

#[inline_macros]
mod in_macro {
    use proc_macros::external;

    // lint local macro expands to function with needless lifetimes
    inline! {
        fn one_input<'a>(x: &'a u8) -> &'a u8 {
        //~^ needless_lifetimes
            unimplemented!()
        }
    }

    // no lint on external macro
    external! {
        fn needless_lifetime<'a>(x: &'a u8) -> &'a u8 {
            unimplemented!()
        }
    }

    inline! {
        fn f<$'a>(arg: &$'a str) -> &$'a str {
            arg
        }
    }
}

mod issue5787 {
    use std::sync::MutexGuard;

    struct Foo;

    impl Foo {
        // doesn't get linted without async
        pub async fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
            guard
        }
    }

    async fn foo<'a>(_x: &i32, y: &'a str) -> &'a str {
        y
    }
}

// https://github.com/rust-lang/rust-clippy/pull/13286#issuecomment-2374245772
mod rayon {
    trait ParallelIterator {
        type Item;
    }

    struct Copied<I: ParallelIterator> {
        base: I,
    }

    impl<'a, T, I> ParallelIterator for Copied<I>
    where
        I: ParallelIterator<Item = &'a T>,
        T: 'a + Copy + Send + Sync,
    {
        type Item = T;
    }
}

mod issue13749 {
    pub struct Generic<T>(T);
    // Non elidable lifetime
    #[expect(clippy::extra_unused_lifetimes)]
    impl<'a, T> Generic<T> where T: 'a {}
}

mod issue13749bis {
    pub struct Generic<T>(T);
    // Non elidable lifetime
    #[expect(clippy::extra_unused_lifetimes)]
    impl<'a, T: 'a> Generic<T> {}
}

fn main() {}
