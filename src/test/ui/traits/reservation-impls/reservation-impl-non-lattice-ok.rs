// build-pass

// Check that a reservation impl does not force other impls to follow
// a lattice discipline.

// Why did we ever want to do this?
//
// We want to eventually add a `impl<T> From<!> for T` impl. That impl conflicts
// with existing impls - at least the `impl<T> From<T> for T` impl. There are
// 2 ways we thought of for dealing with that conflict:
//
// 1. Using specialization and doing some handling for the overlap. The current
// thought is for something like "lattice specialization", which means providing
// an (higher-priority) impl for the intersection of every 2 conflicting impls
// that determines what happens in the intersection case. That's the first
// thing we thought about - see e.g.
// https://github.com/rust-lang/rust/issues/57012#issuecomment-452150775
//
// 2. The other way is to notice that `impl From<!> for T` is basically a marker
// trait, as you say since its only method is uninhabited, and allow for "marker
// trait overlap", where the conflict "doesn't matter" as there is nothing that
// can cause a conflict.
//
// Now it turned out lattice specialization doesn't work it, because an
// `impl<T> From<T> for Smaht<T>` would require a `impl From<!> for Smaht<!>`,
// breaking backwards-compatibility in a fairly painful way. So if we want to
// go with a known approach, we should go with a "marker trait overlap"-style
// approach.

#![feature(rustc_attrs, never_type)]

trait MyTrait {}

impl MyTrait for ! {}

trait MyFrom<T> {
    fn my_from(x: T) -> Self;
}

// Given the "normal" impls for From
#[rustc_reservation_impl="this impl is reserved"]
impl<T> MyFrom<!> for T {
    fn my_from(x: !) -> Self { match x {} }
}

impl<T> MyFrom<T> for T {
    fn my_from(x: T) -> Self { x }
}

// ... we *do* want to allow this common pattern, of `From<!> for MySmaht<T>`
struct MySmaht<T>(T);
impl<T> MyFrom<T> for MySmaht<T> {
    fn my_from(x: T) -> Self { MySmaht(x) }
}

fn main() {}
