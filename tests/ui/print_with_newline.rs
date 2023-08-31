// FIXME: Ideally these suggestions would be fixed via rustfix. Blocked by rust-lang/rust#53934

#![allow(clippy::print_literal)]
#![warn(clippy::print_with_newline)]

fn main() {
    print!("Hello\n");
    //~^ ERROR: using `print!()` with a format string that ends in a single newline
    //~| NOTE: `-D clippy::print-with-newline` implied by `-D warnings`
    print!("Hello {}\n", "world");
    //~^ ERROR: using `print!()` with a format string that ends in a single newline
    print!("Hello {} {}\n", "world", "#2");
    //~^ ERROR: using `print!()` with a format string that ends in a single newline
    print!("{}\n", 1265);
    //~^ ERROR: using `print!()` with a format string that ends in a single newline
    print!("\n");
    //~^ ERROR: using `print!()` with a format string that ends in a single newline

    // these are all fine
    print!("");
    print!("Hello");
    println!("Hello");
    println!("Hello\n");
    println!("Hello {}\n", "world");
    print!("Issue\n{}", 1265);
    print!("{}", 1265);
    print!("\n{}", 1275);
    print!("\n\n");
    print!("like eof\n\n");
    print!("Hello {} {}\n\n", "world", "#2");
    // #3126
    println!("\ndon't\nwarn\nfor\nmultiple\nnewlines\n");
    // #3126
    println!("\nbla\n\n");

    // Escaping
    // #3514
    print!("\\n");
    print!("\\\n");
    //~^ ERROR: using `print!()` with a format string that ends in a single newline
    print!("\\\\n");

    // Raw strings
    // #3778
    print!(r"\n");

    // Literal newlines should also fail
    print!(
        //~^ ERROR: using `print!()` with a format string that ends in a single newline
        "
"
    );
    print!(
        //~^ ERROR: using `print!()` with a format string that ends in a single newline
        r"
"
    );

    // Don't warn on CRLF (#4208)
    print!("\r\n");
    print!("foo\r\n");
    // should fail
    print!("\\r\n");
    //~^ ERROR: using `print!()` with a format string that ends in a single newline
    print!("foo\rbar\n");

    // Ignore expanded format strings
    macro_rules! newline {
        () => {
            "\n"
        };
    }
    print!(newline!());
}
