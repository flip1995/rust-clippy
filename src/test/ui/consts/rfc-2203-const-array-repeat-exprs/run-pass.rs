// run-pass
#![feature(const_in_array_repeat_expressions)]

#[derive(Debug, Eq, PartialEq)]
struct Bar;

fn main() {
    const FOO: Option<Bar> = None;
    const ARR: [Option<Bar>; 2] = [FOO; 2];

    assert_eq!(ARR, [None::<Bar>, None::<Bar>]);
}
