#![warn(clippy::transmute_float_to_int)]

fn float_to_int() {
    let _: u32 = unsafe { std::mem::transmute(1f32) };
    //~^ ERROR: transmute from a `f32` to a `u32`
    //~| NOTE: `-D clippy::transmute-float-to-int` implied by `-D warnings`
    let _: i32 = unsafe { std::mem::transmute(1f32) };
    //~^ ERROR: transmute from a `f32` to a `i32`
    let _: u64 = unsafe { std::mem::transmute(1f64) };
    //~^ ERROR: transmute from a `f64` to a `u64`
    let _: i64 = unsafe { std::mem::transmute(1f64) };
    //~^ ERROR: transmute from a `f64` to a `i64`
    let _: u64 = unsafe { std::mem::transmute(1.0) };
    //~^ ERROR: transmute from a `f64` to a `u64`
    let _: u64 = unsafe { std::mem::transmute(-1.0) };
    //~^ ERROR: transmute from a `f64` to a `u64`
}

mod issue_5747 {
    const VALUE32: i32 = unsafe { std::mem::transmute(1f32) };
    const VALUE64: u64 = unsafe { std::mem::transmute(1f64) };

    const fn to_bits_32(v: f32) -> u32 {
        unsafe { std::mem::transmute(v) }
    }

    const fn to_bits_64(v: f64) -> i64 {
        unsafe { std::mem::transmute(v) }
    }
}

fn main() {}
