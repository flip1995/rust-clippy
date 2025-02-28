#![warn(clippy::all)]
#![warn(clippy::if_not_else)]

fn foo() -> bool {
    unimplemented!()
}
fn bla() -> bool {
    unimplemented!()
}

fn main() {
    if !bla() {
        //~^ if_not_else

        println!("Bugs");
    } else {
        println!("Bunny");
    }
    if 4 != 5 {
        //~^ if_not_else

        println!("Bugs");
    } else {
        println!("Bunny");
    }
    if !foo() {
        println!("Foo");
    } else if !bla() {
        println!("Bugs");
    } else {
        println!("Bunny");
    }

    if !(foo() && bla()) {
        //~^ if_not_else
        #[cfg(not(debug_assertions))]
        println!("not debug");
        #[cfg(debug_assertions)]
        println!("debug");
        if foo() {
            println!("foo");
        } else if bla() {
            println!("bla");
        } else {
            println!("both false");
        }
    } else {
        println!("both true");
    }
}

fn with_comments() {
    if !foo() {
        //~^ if_not_else
        /* foo is false */
        println!("foo is false");
    } else {
        println!("foo"); /* foo */
    }

    if !bla() {
        //~^ if_not_else
        // bla is false
        println!("bla");
    } else {
        println!("bla"); // bla
    }
}

fn with_annotations() {
    #[cfg(debug_assertions)]
    if !foo() {
        //~^ if_not_else
        /* foo is false */
        println!("foo is false");
    } else {
        println!("foo"); /* foo */
    }
}
