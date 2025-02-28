#![warn(clippy::partialeq_to_none)]
#![allow(clippy::eq_op, clippy::needless_if)]

struct Foobar;

impl PartialEq<Option<()>> for Foobar {
    fn eq(&self, _: &Option<()>) -> bool {
        false
    }
}

#[allow(dead_code)]
fn foo(f: Option<u32>) -> &'static str {
    if f != None { "yay" } else { "nay" }
    //~^ partialeq_to_none
}

fn foobar() -> Option<()> {
    None
}

fn bar() -> Result<(), ()> {
    Ok(())
}

fn optref() -> &'static &'static Option<()> {
    &&None
}

pub fn macro_expansion() {
    macro_rules! foo {
        () => {
            None::<()>
        };
    }

    let _ = foobar() == foo!();
    let _ = foo!() == foobar();
    let _ = foo!() == foo!();
}

fn main() {
    let x = Some(0);

    let _ = x == None;
    //~^ partialeq_to_none
    let _ = x != None;
    //~^ partialeq_to_none
    let _ = None == x;
    //~^ partialeq_to_none
    let _ = None != x;
    //~^ partialeq_to_none

    if foobar() == None {}
    //~^ partialeq_to_none

    if bar().ok() != None {}
    //~^ partialeq_to_none

    let _ = Some(1 + 2) != None;
    //~^ partialeq_to_none

    let _ = { Some(0) } == None;
    //~^ partialeq_to_none

    let _ = {
        //~^ partialeq_to_none
        /*
          This comment runs long
        */
        Some(1)
    } != None;

    // Should not trigger, as `Foobar` is not an `Option` and has no `is_none`
    let _ = Foobar == None;

    let _ = optref() == &&None;
    //~^ partialeq_to_none
    let _ = &&None != optref();
    //~^ partialeq_to_none
    let _ = **optref() == None;
    //~^ partialeq_to_none
    let _ = &None != *optref();
    //~^ partialeq_to_none

    let x = Box::new(Option::<()>::None);
    let _ = None != *x;
    //~^ partialeq_to_none
}
