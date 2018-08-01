#![warn(deprecated_cfg_attr)]

#![cfg_attr(feature="cargo-clippy", allow(absurd_extreme_comparisons))]

#[cfg_attr(feature="cargo-clippy", warn(absurd_extreme_comparisons, almost_swapped))]
fn main() {}
