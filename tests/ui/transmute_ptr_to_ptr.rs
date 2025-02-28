#![warn(clippy::transmute_ptr_to_ptr)]
#![allow(clippy::borrow_as_ptr, clippy::missing_transmute_annotations)]

use std::mem::transmute;

// Make sure we can modify lifetimes, which is one of the recommended uses
// of transmute

// Make sure we can do static lifetime transmutes
unsafe fn transmute_lifetime_to_static<'a, T>(t: &'a T) -> &'static T {
    transmute::<&'a T, &'static T>(t)
}

// Make sure we can do non-static lifetime transmutes
unsafe fn transmute_lifetime<'a, 'b, T>(t: &'a T, u: &'b T) -> &'b T {
    transmute::<&'a T, &'b T>(t)
}

struct LifetimeParam<'a> {
    s: &'a str,
}

struct GenericParam<T> {
    t: T,
}

fn transmute_ptr_to_ptr() {
    let ptr = &1u32 as *const u32;
    let mut_ptr = &mut 1u32 as *mut u32;
    unsafe {
        // pointer-to-pointer transmutes; bad
        let _: *const f32 = transmute(ptr);
        //~^ transmute_ptr_to_ptr

        let _: *mut f32 = transmute(mut_ptr);
        //~^ transmute_ptr_to_ptr

        // ref-ref transmutes; bad
        let _: &f32 = transmute(&1u32);
        //~^ transmute_ptr_to_ptr

        let _: &f32 = transmute(&1f64);
        //~^ transmute_ptr_to_ptr

        //:^ this test is here because both f32 and f64 are the same TypeVariant, but they are not
        // the same type
        let _: &mut f32 = transmute(&mut 1u32);
        //~^ transmute_ptr_to_ptr

        let _: &GenericParam<f32> = transmute(&GenericParam { t: 1u32 });
        //~^ transmute_ptr_to_ptr

        let u64_ref: &u64 = &0u64;
        let u8_ref: &u8 = transmute(u64_ref);
        //~^ transmute_ptr_to_ptr

        let _: *const u32 = transmute(mut_ptr);
        //~^ transmute_ptr_to_ptr

        let _: *mut u32 = transmute(ptr);
        //~^ transmute_ptr_to_ptr
    }

    // transmute internal lifetimes, should not lint
    let s = "hello world".to_owned();
    let lp = LifetimeParam { s: &s };
    let _: &LifetimeParam<'static> = unsafe { transmute(&lp) };
    let _: &GenericParam<&LifetimeParam<'static>> = unsafe { transmute(&GenericParam { t: &lp }) };
}

fn lifetime_to_static(v: *mut &()) -> *const &'static () {
    unsafe { transmute(v) }
    //~^ transmute_ptr_to_ptr
}

// dereferencing raw pointers in const contexts, should not lint as it's unstable (issue 5959)
const _: &() = {
    struct Zst;
    let zst = &Zst;

    unsafe { transmute::<&'static Zst, &'static ()>(zst) }
};

#[clippy::msrv = "1.37"]
fn msrv_1_37(ptr: *const u8) {
    unsafe {
        let _: *const i8 = transmute(ptr);
        //~^ transmute_ptr_to_ptr
    }
}

#[clippy::msrv = "1.38"]
fn msrv_1_38(ptr: *const u8) {
    unsafe {
        let _: *const i8 = transmute(ptr);
        //~^ transmute_ptr_to_ptr
    }
}

#[clippy::msrv = "1.64"]
fn msrv_1_64(ptr: *const u8, mut_ptr: *mut u8) {
    unsafe {
        let _: *mut u8 = transmute(ptr);
        //~^ transmute_ptr_to_ptr
        let _: *const u8 = transmute(mut_ptr);
        //~^ transmute_ptr_to_ptr
    }
}

#[clippy::msrv = "1.65"]
fn msrv_1_65(ptr: *const u8, mut_ptr: *mut u8) {
    unsafe {
        let _: *mut u8 = transmute(ptr);
        //~^ transmute_ptr_to_ptr
        let _: *const u8 = transmute(mut_ptr);
        //~^ transmute_ptr_to_ptr
    }
}

fn main() {}
