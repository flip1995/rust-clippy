// aux-build:invalid-punct-ident.rs

// FIXME https://github.com/rust-lang/rust/issues/59998
// normalize-stderr-test "thread.*panicked.*proc_macro_server.rs.*\n" -> ""
// normalize-stderr-test "note:.*RUST_BACKTRACE=1.*\n" -> ""
// normalize-stderr-test "error: internal compiler error.*\n" -> ""
// normalize-stderr-test "note:.*unexpectedly panicked.*\n" -> ""
// normalize-stderr-test "note: we would appreciate a bug report.*\n" -> ""
// normalize-stderr-test "note: compiler flags.*\n" -> ""
// normalize-stderr-test "note: rustc.*running on.*\n" -> ""

#[macro_use]
extern crate invalid_punct_ident;

invalid_ident!(); //~ ERROR proc macro panicked
