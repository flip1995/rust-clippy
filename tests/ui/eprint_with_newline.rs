#![allow(clippy::print_literal)]
#![warn(clippy::print_with_newline)]

fn main() {
    eprint!("Hello\n");
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
    //~| NOTE: `-D clippy::print-with-newline` implied by `-D warnings`
    eprint!("Hello {}\n", "world");
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
    eprint!("Hello {} {}\n", "world", "#2");
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
    eprint!("{}\n", 1265);
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
    eprint!("\n");
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline

    // these are all fine
    eprint!("");
    eprint!("Hello");
    eprintln!("Hello");
    eprintln!("Hello\n");
    eprintln!("Hello {}\n", "world");
    eprint!("Issue\n{}", 1265);
    eprint!("{}", 1265);
    eprint!("\n{}", 1275);
    eprint!("\n\n");
    eprint!("like eof\n\n");
    eprint!("Hello {} {}\n\n", "world", "#2");
    // #3126
    eprintln!("\ndon't\nwarn\nfor\nmultiple\nnewlines\n");
    // #3126
    eprintln!("\nbla\n\n");

    // Escaping
    // #3514
    eprint!("\\n");
    eprint!("\\\n");
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
    eprint!("\\\\n");

    // Raw strings
    // #3778
    eprint!(r"\n");

    // Literal newlines should also fail
    eprint!(
        //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
        "
"
    );
    eprint!(
        //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
        r"
"
    );

    // Don't warn on CRLF (#4208)
    eprint!("\r\n");
    eprint!("foo\r\n");
    eprint!("\\r\n");
    //~^ ERROR: using `eprint!()` with a format string that ends in a single newline
    eprint!("foo\rbar\n");

    // Ignore expanded format strings
    macro_rules! newline {
        () => {
            "\n"
        };
    }
    eprint!(newline!());
}
