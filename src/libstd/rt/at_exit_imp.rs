// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of running at_exit routines
//!
//! Documentation can be found on the `rt::at_exit` function.

use core::prelude::*;

use libc;
use boxed::Box;
use vec::Vec;
use sync::{atomic, Once, ONCE_INIT};
use mem;
use thunk::Thunk;

use rt::exclusive::Exclusive;

type Queue = Exclusive<Vec<Thunk>>;

static INIT: Once = ONCE_INIT;
static QUEUE: atomic::AtomicUint = atomic::INIT_ATOMIC_UINT;
static RUNNING: atomic::AtomicBool = atomic::INIT_ATOMIC_BOOL;

fn init() {
    let state: Box<Queue> = box Exclusive::new(Vec::new());
    unsafe {
        QUEUE.store(mem::transmute(state), atomic::SeqCst);
        libc::atexit(run);
    }
}

// Note: this is private and so can only be called via atexit above,
// which guarantees initialization.
extern fn run() {
    let cur = unsafe {
        rtassert!(!RUNNING.load(atomic::SeqCst));
        let queue = QUEUE.swap(0, atomic::SeqCst);
        rtassert!(queue != 0);

        let queue: Box<Queue> = mem::transmute(queue);
        let v = mem::replace(&mut *queue.lock(), Vec::new());
        v
    };

    for to_run in cur.into_iter() {
        to_run.invoke(());
    }
}

pub fn push(f: Thunk) {
    INIT.doit(init);
    unsafe {
        // Note that the check against 0 for the queue pointer is not atomic at
        // all with respect to `run`, meaning that this could theoretically be a
        // use-after-free. There's not much we can do to protect against that,
        // however. Let's just assume a well-behaved runtime and go from there!
        rtassert!(!RUNNING.load(atomic::SeqCst));
        let queue = QUEUE.load(atomic::SeqCst);
        rtassert!(queue != 0);
        (*(queue as *const Queue)).lock().push(f);
    }
}
