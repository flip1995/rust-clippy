#![feature(plugin)]
#![plugin(clippy)]

#![deny(while_let_loop)]
#![allow(dead_code, unused)]

fn main() {
    let y = Some(true);
    loop { //~ERROR
        if let Some(_x) = y {
            let _v = 1;
        } else {
            break
        }
    }
    loop { // no error, break is not in else clause
        if let Some(_x) = y {
            let _v = 1;
        }
        break;
    }
    loop { //~ERROR
        match y {
            Some(_x) => true,
            None => break
        };
    }
    loop { //~ERROR
        let x = match y {
            Some(x) => x,
            None => break
        };
        let _x = x;
        let _str = "foo";
    }
    loop { // no error, else branch does something other than break
        match y {
            Some(_x) => true,
            _ => {
                let _z = 1;
                break;
            }
        };
    }
    while let Some(x) = y { // no error, obviously
        println!("{}", x);
    }
}

// regression test (#360)
// this should not panic
// it's okay if further iterations of the lint
// cause this function to trigger it
fn no_panic<T>(slice: &[T]) {
    let mut iter = slice.iter();
    loop {
        let _ = match iter.next() {
            Some(ele) => ele,
            None => break
        };
        loop {}
    }
}
