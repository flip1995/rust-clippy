// compile-flags: --crate-name=rust_version
#![warn(clippy::approx_constant)]

fn main() {
    let log2_10 = 3.321928094887362; // should trigger approx_constant
}
