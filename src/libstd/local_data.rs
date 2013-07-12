// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

Task local data management

Allows storing arbitrary types inside task-local-storage (TLS), to be accessed
anywhere within a task, keyed by a global slice of the appropriate type.
Useful for dynamic variables, singletons, and interfacing with foreign code
with bad callback interfaces.

To use, declare a static slice of the type you wish to store. The initialization
should be `&[]`. This is then the key to what you wish to store.

~~~{.rust}
use std::local_data;

static key_int: local_data::Key<int> = &[];
static key_vector: local_data::Key<~[int]> = &[];

local_data::set(key_int, 3);
local_data::get(key_int, |opt| assert_eq!(opt, Some(&3)));

local_data::set(key_vector, ~[4]);
local_data::get(key_int, |opt| assert_eq!(opt, Some(&~[4])));
~~~

Casting 'Arcane Sight' reveals an overwhelming aura of Transmutation
magic.

*/

use prelude::*;

use task::local_data_priv::{local_get, local_pop, local_set, Handle};

#[cfg(test)] use task;

/**
 * Indexes a task-local data slot. The function's code pointer is used for
 * comparison. Recommended use is to write an empty function for each desired
 * task-local data slot (and use class destructors, not code inside the
 * function, if specific teardown is needed). DO NOT use multiple
 * instantiations of a single polymorphic function to index data of different
 * types; arbitrary type coercion is possible this way.
 *
 * One other exception is that this global state can be used in a destructor
 * context to create a circular @-box reference, which will crash during task
 * failure (see issue #3039).
 *
 * These two cases aside, the interface is safe.
 */
#[cfg(not(stage0))]
pub type Key<T> = &'static [T];
#[cfg(stage0)]
pub type Key<'self,T> = &'self fn:Copy(v: T);

/**
 * Remove a task-local data value from the table, returning the
 * reference that was originally created to insert it.
 */
#[cfg(stage0)]
pub fn pop<T: 'static>(key: Key<@T>) -> Option<@T> {
    unsafe { local_pop(Handle::new(), key) }
}
/**
 * Remove a task-local data value from the table, returning the
 * reference that was originally created to insert it.
 */
#[cfg(not(stage0))]
pub fn pop<T: 'static>(key: Key<T>) -> Option<T> {
    unsafe { local_pop(Handle::new(), key) }
}
/**
 * Retrieve a task-local data value. It will also be kept alive in the
 * table until explicitly removed.
 */
#[cfg(stage0)]
pub fn get<T: 'static, U>(key: Key<@T>, f: &fn(Option<&@T>) -> U) -> U {
    unsafe { local_get(Handle::new(), key, f) }
}
/**
 * Retrieve a task-local data value. It will also be kept alive in the
 * table until explicitly removed.
 */
#[cfg(not(stage0))]
pub fn get<T: 'static, U>(key: Key<T>, f: &fn(Option<&T>) -> U) -> U {
    unsafe { local_get(Handle::new(), key, f) }
}
/**
 * Store a value in task-local data. If this key already has a value,
 * that value is overwritten (and its destructor is run).
 */
#[cfg(stage0)]
pub fn set<T: 'static>(key: Key<@T>, data: @T) {
    unsafe { local_set(Handle::new(), key, data) }
}
/**
 * Store a value in task-local data. If this key already has a value,
 * that value is overwritten (and its destructor is run).
 */
#[cfg(not(stage0))]
pub fn set<T: 'static>(key: Key<T>, data: T) {
    unsafe { local_set(Handle::new(), key, data) }
}
/**
 * Modify a task-local data value. If the function returns 'None', the
 * data is removed (and its reference dropped).
 */
#[cfg(stage0)]
pub fn modify<T: 'static>(key: Key<@T>, f: &fn(Option<@T>) -> Option<@T>) {
    match f(pop(key)) {
        Some(next) => { set(key, next); }
        None => {}
    }
}
/**
 * Modify a task-local data value. If the function returns 'None', the
 * data is removed (and its reference dropped).
 */
#[cfg(not(stage0))]
pub fn modify<T: 'static>(key: Key<T>, f: &fn(Option<T>) -> Option<T>) {
    match f(pop(key)) {
        Some(next) => { set(key, next); }
        None => {}
    }
}

#[test]
fn test_tls_multitask() {
    static my_key: Key<@~str> = &[];
    set(my_key, @~"parent data");
    do task::spawn {
        // TLS shouldn't carry over.
        assert!(get(my_key, |k| k.map(|&k| *k)).is_none());
        set(my_key, @~"child data");
        assert!(*(get(my_key, |k| k.map(|&k| *k)).get()) ==
                ~"child data");
        // should be cleaned up for us
    }
    // Must work multiple times
    assert!(*(get(my_key, |k| k.map(|&k| *k)).get()) == ~"parent data");
    assert!(*(get(my_key, |k| k.map(|&k| *k)).get()) == ~"parent data");
    assert!(*(get(my_key, |k| k.map(|&k| *k)).get()) == ~"parent data");
}

#[test]
fn test_tls_overwrite() {
    static my_key: Key<@~str> = &[];
    set(my_key, @~"first data");
    set(my_key, @~"next data"); // Shouldn't leak.
    assert!(*(get(my_key, |k| k.map(|&k| *k)).get()) == ~"next data");
}

#[test]
fn test_tls_pop() {
    static my_key: Key<@~str> = &[];
    set(my_key, @~"weasel");
    assert!(*(pop(my_key).get()) == ~"weasel");
    // Pop must remove the data from the map.
    assert!(pop(my_key).is_none());
}

#[test]
fn test_tls_modify() {
    static my_key: Key<@~str> = &[];
    modify(my_key, |data| {
        match data {
            Some(@ref val) => fail!("unwelcome value: %s", *val),
            None           => Some(@~"first data")
        }
    });
    modify(my_key, |data| {
        match data {
            Some(@~"first data") => Some(@~"next data"),
            Some(@ref val)       => fail!("wrong value: %s", *val),
            None                 => fail!("missing value")
        }
    });
    assert!(*(pop(my_key).get()) == ~"next data");
}

#[test]
fn test_tls_crust_automorestack_memorial_bug() {
    // This might result in a stack-canary clobber if the runtime fails to
    // set sp_limit to 0 when calling the cleanup extern - it might
    // automatically jump over to the rust stack, which causes next_c_sp
    // to get recorded as something within a rust stack segment. Then a
    // subsequent upcall (esp. for logging, think vsnprintf) would run on
    // a stack smaller than 1 MB.
    static my_key: Key<@~str> = &[];
    do task::spawn {
        set(my_key, @~"hax");
    }
}

#[test]
fn test_tls_multiple_types() {
    static str_key: Key<@~str> = &[];
    static box_key: Key<@@()> = &[];
    static int_key: Key<@int> = &[];
    do task::spawn {
        set(str_key, @~"string data");
        set(box_key, @@());
        set(int_key, @42);
    }
}

#[test]
fn test_tls_overwrite_multiple_types() {
    static str_key: Key<@~str> = &[];
    static box_key: Key<@@()> = &[];
    static int_key: Key<@int> = &[];
    do task::spawn {
        set(str_key, @~"string data");
        set(int_key, @42);
        // This could cause a segfault if overwriting-destruction is done
        // with the crazy polymorphic transmute rather than the provided
        // finaliser.
        set(int_key, @31337);
    }
}

#[test]
#[should_fail]
#[ignore(cfg(windows))]
fn test_tls_cleanup_on_failure() {
    static str_key: Key<@~str> = &[];
    static box_key: Key<@@()> = &[];
    static int_key: Key<@int> = &[];
    set(str_key, @~"parent data");
    set(box_key, @@());
    do task::spawn {
        // spawn_linked
        set(str_key, @~"string data");
        set(box_key, @@());
        set(int_key, @42);
        fail!();
    }
    // Not quite nondeterministic.
    set(int_key, @31337);
    fail!();
}

#[test]
fn test_static_pointer() {
    static key: Key<@&'static int> = &[];
    static VALUE: int = 0;
    let v: @&'static int = @&VALUE;
    set(key, v);
}

#[test]
fn test_owned() {
    static key: Key<~int> = &[];
    set(key, ~1);
}

#[test]
fn test_same_key_type() {
    static key1: Key<int> = &[];
    static key2: Key<int> = &[];
    static key3: Key<int> = &[];
    static key4: Key<int> = &[];
    static key5: Key<int> = &[];
    set(key1, 1);
    set(key2, 2);
    set(key3, 3);
    set(key4, 4);
    set(key5, 5);

    get(key1, |x| assert_eq!(*x.unwrap(), 1));
    get(key2, |x| assert_eq!(*x.unwrap(), 2));
    get(key3, |x| assert_eq!(*x.unwrap(), 3));
    get(key4, |x| assert_eq!(*x.unwrap(), 4));
    get(key5, |x| assert_eq!(*x.unwrap(), 5));
}
