#![feature(rustc_attrs, const_let, const_fn)]

#[rustc_layout_scalar_valid_range_start(1)]
#[repr(transparent)]
pub(crate) struct NonZero<T>(pub(crate) T);
fn main() {}

const fn foo() -> NonZero<u32> {
    let mut x = unsafe { NonZero(1) };
    x.0 = 0; //~ ERROR statements in constant functions are unstable
    //~^ ERROR mutation of layout constrained field is unsafe
    x
}

const fn bar() -> NonZero<u32> {
    let mut x = unsafe { NonZero(1) };
    unsafe { x.0 = 0 }; //~ ERROR statements in constant functions are unstable
    x
}
