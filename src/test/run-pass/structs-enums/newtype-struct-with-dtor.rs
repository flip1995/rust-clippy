// run-pass
#![allow(unused_unsafe)]
#![allow(unused_variables)]
// pretty-expanded FIXME #23616

pub struct Fd(u32);

fn foo(a: u32) {}

impl Drop for Fd {
    fn drop(&mut self) {
        unsafe {
            let Fd(s) = *self;
            foo(s);
        }
    }
}

pub fn main() {
}
