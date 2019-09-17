#![allow(dead_code)]
#![warn(clippy::unused_underscore_arg)]

// This is ok, because the function is public
pub fn unused_in_pub(_s: u32) {}

// Don't lint trait functions
trait UnderscoreInTrait {
    fn underscore_ok(_s: u32) {}
}

struct Dummy;

// Also don't lint trait impls
impl UnderscoreInTrait for Dummy {
    fn underscore_ok(_s: u32) {}
}

struct HasDrop;

impl Drop for HasDrop {
    fn drop(&mut self) {
        println!("mic");
    }
}

// Don't lint arguments with a drop impl
fn unused_with_drop(_s: HasDrop) {}

impl Dummy {
    // This is ok
    pub fn unused_in_pub_impl(_s: u32) {}
    // This is ok
    fn unused_with_drop_in_impl(_s: HasDrop) {}
    fn unused_in_private_impl(_s: u32) {} //~ ERROR: _s can be removed
}

fn unused_in_private(_s: u32) {} //~ ERROR: _s can be removed

fn main() {}
