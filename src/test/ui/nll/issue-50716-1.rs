//
// An additional regression test for the issue #50716 “NLL ignores lifetimes
// bounds derived from `Sized` requirements” that checks that the fixed compiler
// accepts this code fragment with both AST and MIR borrow checkers.
//
// build-pass (FIXME(62277): could be check-pass?)

struct Qey<Q: ?Sized>(Q);

fn main() {}
