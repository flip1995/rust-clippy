// run-pass
use std::fmt::Display;

static BYTE: u8 = 33;

fn main() {
    let x: &('static + Display) = &BYTE;
    let y: Box<'static + Display> = Box::new(BYTE);
    let xstr = format!("{}", x);
    let ystr = format!("{}", y);
    assert_eq!(xstr, "33");
    assert_eq!(ystr, "33");
}
