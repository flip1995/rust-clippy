#![warn(clippy::drain_collect)]
#![no_std]
extern crate alloc;
use alloc::vec::Vec;

fn remove_all(v: &mut Vec<i32>) -> Vec<i32> {
    v.drain(..).collect()
    //~^ drain_collect
}
