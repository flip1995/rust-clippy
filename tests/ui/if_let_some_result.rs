// run-rustfix

#![warn(clippy::if_let_some_result)]
#![allow(dead_code)]

fn str_to_int(x: &str) -> i32 {
    if let Some(y) = x.parse().ok() { y } else { 0 }
}

fn str_to_int_ok(x: &str) -> i32 {
    if let Ok(y) = x.parse() { y } else { 0 }
}

#[rustfmt::skip]
fn strange_some_no_else(x: &str) -> i32 {
    {
        if let Some(y) = x   .   parse()   .   ok   ()    {
            return y;
        };
        0
    }
}

fn negative() {
    while let Some(1) = "".parse().ok() {}
}

fn main() {}
