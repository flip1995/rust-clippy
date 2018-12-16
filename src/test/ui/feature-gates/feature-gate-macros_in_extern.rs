#![feature(decl_macro)]

macro_rules! returns_isize(
    ($ident:ident) => (
        fn $ident() -> isize;
    )
);

macro takes_u32_returns_u32($ident:ident) {
    fn $ident (arg: u32) -> u32;
}

macro_rules! emits_nothing(
    () => ()
);

#[link(name = "rust_test_helpers", kind = "static")]
extern {
    returns_isize!(rust_get_test_int);
    //~^ ERROR macro invocations in `extern {}` blocks are experimental
    takes_u32_returns_u32!(rust_dbg_extern_identity_u32);
    //~^ ERROR macro invocations in `extern {}` blocks are experimental
    emits_nothing!();
    //~^ ERROR macro invocations in `extern {}` blocks are experimental
}

fn main() {}
