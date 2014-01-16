// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[doc(hidden)];

use libc::c_void;
use ptr;
use unstable::intrinsics::TyDesc;
use unstable::raw;

type DropGlue<'a> = 'a |**TyDesc, *c_void|;

static RC_IMMORTAL : uint = 0x77777777;

/*
 * Box annihilation
 *
 * This runs at task death to free all boxes.
 */

struct AnnihilateStats {
    n_total_boxes: uint,
    n_bytes_freed: uint
}

unsafe fn each_live_alloc(read_next_before: bool,
                          f: |alloc: *mut raw::Box<()>| -> bool)
                          -> bool {
    //! Walks the internal list of allocations

    use rt::local_heap;

    let mut alloc = local_heap::live_allocs();
    while alloc != ptr::mut_null() {
        let next_before = (*alloc).next;

        if !f(alloc) {
            return false;
        }

        if read_next_before {
            alloc = next_before;
        } else {
            alloc = (*alloc).next;
        }
    }
    return true;
}

#[cfg(unix)]
fn debug_mem() -> bool {
    // XXX: Need to port the environment struct to newsched
    false
}

#[cfg(windows)]
fn debug_mem() -> bool {
    false
}

/// Destroys all managed memory (i.e. @ boxes) held by the current task.
pub unsafe fn annihilate() {
    use rt::local_heap::local_free;
    use mem;

    let mut stats = AnnihilateStats {
        n_total_boxes: 0,
        n_bytes_freed: 0
    };

    // Pass 1: Make all boxes immortal.
    //
    // In this pass, nothing gets freed, so it does not matter whether
    // we read the next field before or after the callback.
    each_live_alloc(true, |alloc| {
        stats.n_total_boxes += 1;
        (*alloc).ref_count = RC_IMMORTAL;
        true
    });

    // Pass 2: Drop all boxes.
    //
    // In this pass, unique-managed boxes may get freed, but not
    // managed boxes, so we must read the `next` field *after* the
    // callback, as the original value may have been freed.
    each_live_alloc(false, |alloc| {
        let tydesc = (*alloc).type_desc;
        let data = &(*alloc).data as *();
        ((*tydesc).drop_glue)(data as *i8);
        true
    });

    // Pass 3: Free all boxes.
    //
    // In this pass, managed boxes may get freed (but not
    // unique-managed boxes, though I think that none of those are
    // left), so we must read the `next` field before, since it will
    // not be valid after.
    each_live_alloc(true, |alloc| {
        stats.n_bytes_freed +=
            (*((*alloc).type_desc)).size
            + mem::size_of::<raw::Box<()>>();
        local_free(alloc as *i8);
        true
    });

    if debug_mem() {
        // We do logging here w/o allocation.
        debug!("annihilator stats:\n  \
                       total boxes: {}\n  \
                       bytes freed: {}",
                stats.n_total_boxes, stats.n_bytes_freed);
    }
}
