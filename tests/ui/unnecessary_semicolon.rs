//@revisions: edition2021 edition2024
//@[edition2021] edition:2021
//@[edition2024] edition:2024

#![warn(clippy::unnecessary_semicolon)]
#![feature(postfix_match)]
#![allow(clippy::single_match)]

fn no_lint(mut x: u32) -> Option<u32> {
    Some(())?;

    {
        let y = 3;
        dbg!(x + y)
    };

    {
        let (mut a, mut b) = (10, 20);
        (a, b) = (b + 1, a + 1);
    }

    Some(0)
}

fn main() {
    let mut a = 3;
    if a == 2 {
        println!("This is weird");
    };
    //~^ unnecessary_semicolon

    a.match {
        3 => println!("three"),
        _ => println!("not three"),
    };
    //~^ unnecessary_semicolon
}

// This is a problem in edition 2021 and below
fn borrow_issue() {
    let v = std::cell::RefCell::new(Some(vec![1]));
    match &*v.borrow() {
        Some(v) => {
            dbg!(v);
        },
        None => {},
    };
    //~[edition2024]^ unnecessary_semicolon
}

fn no_borrow_issue(a: u32, b: u32) {
    match Some(a + b) {
        Some(v) => {
            dbg!(v);
        },
        None => {},
    };
    //~^ unnecessary_semicolon
}

fn issue14100() -> bool {
    // Removing the `;` would make the block type be `()` instead of `!`, and this could no longer be
    // cast into the `bool` function return type.
    if return true {};
}
