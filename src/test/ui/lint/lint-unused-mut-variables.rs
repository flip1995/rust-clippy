// edition:2018

// Exercise the unused_mut attribute in some positive and negative cases

#![deny(unused_mut)]
#![feature(async_closure, raw_ref_op)]

async fn baz_async(
    mut a: i32,
    //~^ ERROR: variable does not need to be mutable
    #[allow(unused_mut)] mut b: i32,
) {}
fn baz(
    mut a: i32,
    //~^ ERROR: variable does not need to be mutable
    #[allow(unused_mut)] mut b: i32,
    #[allow(unused_mut)] (mut c, d): (i32, i32)
) {}

struct RefStruct {}
impl RefStruct {
    async fn baz_async(
        mut a: i32,
        //~^ ERROR: variable does not need to be mutable
        #[allow(unused_mut)] mut b: i32,
    ) {}
    fn baz(
        &self,
        mut a: i32,
        //~^ ERROR: variable does not need to be mutable
        #[allow(unused_mut)] mut b: i32,
        #[allow(unused_mut)] (mut c, d): (i32, i32)
    ) {}
}

trait RefTrait {
    fn baz(
        &self,
        mut a: i32,
        //~^ ERROR: variable does not need to be mutable
        #[allow(unused_mut)] mut b: i32,
        #[allow(unused_mut)] (mut c, d): (i32, i32)
    ) {}
}
impl RefTrait for () {
    fn baz(
        &self,
        mut a: i32,
        //~^ ERROR: variable does not need to be mutable
        #[allow(unused_mut)] mut b: i32,
        #[allow(unused_mut)] (mut c, d): (i32, i32)
    ) {}
}

fn main() {
    let _ = async move |
        mut a: i32,
        //~^ ERROR: variable does not need to be mutable
        #[allow(unused_mut)] mut b: i32,
    | {};
    let _ = |
        mut a: i32,
        //~^ ERROR: variable does not need to be mutable
        #[allow(unused_mut)] mut b: i32,
        #[allow(unused_mut)] (mut c, d): (i32, i32)
    | {};

    // negative cases
    let mut a = 3; //~ ERROR: variable does not need to be mutable

    let mut a = 2; //~ ERROR: variable does not need to be mutable

    let mut b = 3; //~ ERROR: variable does not need to be mutable

    let mut a = vec![3]; //~ ERROR: variable does not need to be mutable

    let (mut a, b) = (1, 2); //~ ERROR: variable does not need to be mutable

    let mut a; //~ ERROR: variable does not need to be mutable

    a = 3;

    let mut b; //~ ERROR: variable does not need to be mutable

    if true {
        b = 3;
    } else {
        b = 4;
    }

    match 30 {
        mut x => {} //~ ERROR: variable does not need to be mutable

    }
    match (30, 2) {
      (mut x, 1) | //~ ERROR: variable does not need to be mutable

      (mut x, 2) |
      (mut x, 3) => {
      }
      _ => {}
    }

    let x = |mut y: isize| 10; //~ ERROR: variable does not need to be mutable

    fn what(mut foo: isize) {} //~ ERROR: variable does not need to be mutable


    let mut a = &mut 5; //~ ERROR: variable does not need to be mutable

    *a = 4;

    let mut a = 5;
    let mut b = (&mut a,); //~ ERROR: variable does not need to be mutable
    *b.0 = 4;

    let mut x = &mut 1; //~ ERROR: variable does not need to be mutable

    let mut f = || {
      *x += 1;
    };
    f();

    fn mut_ref_arg(mut arg : &mut [u8]) -> &mut [u8] {
        &mut arg[..] //~^ ERROR: variable does not need to be mutable

    }

    let mut v : &mut Vec<()> = &mut vec![]; //~ ERROR: variable does not need to be mutable

    v.push(());

    // positive cases
    let mut a = 2;
    a = 3;
    let mut a = Vec::new();
    a.push(3);
    let mut a = Vec::new();
    callback(|| {
        a.push(3);
    });
    let mut a = Vec::new();
    callback(|| {
        callback(|| {
            a.push(3);
        });
    });
    let (mut a, b) = (1, 2);
    a = 34;

    match 30 {
        mut x => {
            x = 21;
        }
    }

    match (30, 2) {
      (mut x, 1) |
      (mut x, 2) |
      (mut x, 3) => {
        x = 21
      }
      _ => {}
    }

    // Attribute should be respected on match arms
    match 0 {
        #[allow(unused_mut)]
        mut x => {
            let mut y = 1;
        },
    }

    let x = |mut y: isize| y = 32;
    fn nothing(mut foo: isize) { foo = 37; }

    // leading underscore should avoid the warning, just like the
    // unused variable lint.
    let mut _allowed = 1;

    let mut raw_address_of_mut = 1; // OK
    let mut_ptr = &raw mut raw_address_of_mut;

    let mut raw_address_of_const = 1; //~ ERROR: variable does not need to be mutable
    let const_ptr = &raw const raw_address_of_const;
}

fn callback<F>(f: F) where F: FnOnce() {}

// make sure the lint attribute can be turned off
#[allow(unused_mut)]
fn foo(mut a: isize) {
    let mut a = 3;
    let mut b = vec![2];
}

// make sure the lint attribute can be turned off on let statements
#[deny(unused_mut)]
fn bar() {
    #[allow(unused_mut)]
    let mut a = 3;
    let mut b = vec![2]; //~ ERROR: variable does not need to be mutable

}
