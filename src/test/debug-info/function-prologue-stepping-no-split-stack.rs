// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-android: FIXME(#10381)

// This test case checks if function arguments already have the correct value when breaking at the
// beginning of a function. Functions with the #[no_split_stack] attribute have the same prologue as
// regular C functions compiled with GCC or Clang and therefore are better handled by GDB. As a
// consequence, and as opposed to regular Rust functions, we can set the breakpoints via the
// function name (and don't have to fall back on using line numbers).

// compile-flags:-g
// debugger:set print pretty off
// debugger:rbreak immediate_args
// debugger:rbreak binding
// debugger:rbreak assignment
// debugger:rbreak function_call
// debugger:rbreak identifier
// debugger:rbreak return_expr
// debugger:rbreak arithmetic_expr
// debugger:rbreak if_expr
// debugger:rbreak while_expr
// debugger:rbreak loop_expr
// debugger:run

// IMMEDIATE ARGS
// debugger:print a
// check:$1 = 1
// debugger:print b
// check:$2 = true
// debugger:print c
// check:$3 = 2.5
// debugger:continue

// NON IMMEDIATE ARGS
// debugger:print a
// check:$4 = {a = 3, b = 4, c = 5, d = 6, e = 7, f = 8, g = 9, h = 10}
// debugger:print b
// check:$5 = {a = 11, b = 12, c = 13, d = 14, e = 15, f = 16, g = 17, h = 18}
// debugger:continue

// BINDING
// debugger:print a
// check:$6 = 19
// debugger:print b
// check:$7 = 20
// debugger:print c
// check:$8 = 21.5
// debugger:continue

// ASSIGNMENT
// debugger:print a
// check:$9 = 22
// debugger:print b
// check:$10 = 23
// debugger:print c
// check:$11 = 24.5
// debugger:continue

// FUNCTION CALL
// debugger:print x
// check:$12 = 25
// debugger:print y
// check:$13 = 26
// debugger:print z
// check:$14 = 27.5
// debugger:continue

// EXPR
// debugger:print x
// check:$15 = 28
// debugger:print y
// check:$16 = 29
// debugger:print z
// check:$17 = 30.5
// debugger:continue

// RETURN EXPR
// debugger:print x
// check:$18 = 31
// debugger:print y
// check:$19 = 32
// debugger:print z
// check:$20 = 33.5
// debugger:continue

// ARITHMETIC EXPR
// debugger:print x
// check:$21 = 34
// debugger:print y
// check:$22 = 35
// debugger:print z
// check:$23 = 36.5
// debugger:continue

// IF EXPR
// debugger:print x
// check:$24 = 37
// debugger:print y
// check:$25 = 38
// debugger:print z
// check:$26 = 39.5
// debugger:continue

// WHILE EXPR
// debugger:print x
// check:$27 = 40
// debugger:print y
// check:$28 = 41
// debugger:print z
// check:$29 = 42
// debugger:continue

// LOOP EXPR
// debugger:print x
// check:$30 = 43
// debugger:print y
// check:$31 = 44
// debugger:print z
// check:$32 = 45
// debugger:continue

#[allow(unused_variable)];

#[no_split_stack]
fn immediate_args(a: int, b: bool, c: f64) {
    ()
}

struct BigStruct {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
    e: u64,
    f: u64,
    g: u64,
    h: u64
}

#[no_split_stack]
fn non_immediate_args(a: BigStruct, b: BigStruct) {
    ()
}

#[no_split_stack]
fn binding(a: i64, b: u64, c: f64) {
    let x = 0;
}

#[no_split_stack]
fn assignment(mut a: u64, b: u64, c: f64) {
    a = b;
}

#[no_split_stack]
fn function_call(x: u64, y: u64, z: f64) {
    std::io::stdio::print("Hi!")
}

#[no_split_stack]
fn identifier(x: u64, y: u64, z: f64) -> u64 {
    x
}

#[no_split_stack]
fn return_expr(x: u64, y: u64, z: f64) -> u64 {
    return x;
}

#[no_split_stack]
fn arithmetic_expr(x: u64, y: u64, z: f64) -> u64 {
    x + y
}

#[no_split_stack]
fn if_expr(x: u64, y: u64, z: f64) -> u64 {
    if x + y < 1000 {
        x
    } else {
        y
    }
}

#[no_split_stack]
fn while_expr(mut x: u64, y: u64, z: u64) -> u64 {
    while x + y < 1000 {
        x += z
    }
    return x;
}

#[no_split_stack]
fn loop_expr(mut x: u64, y: u64, z: u64) -> u64 {
    loop {
        x += z;

        if x + y > 1000 {
            return x;
        }
    }
}

fn main() {
    immediate_args(1, true, 2.5);

    non_immediate_args(
        BigStruct {
            a: 3,
            b: 4,
            c: 5,
            d: 6,
            e: 7,
            f: 8,
            g: 9,
            h: 10
        },
        BigStruct {
            a: 11,
            b: 12,
            c: 13,
            d: 14,
            e: 15,
            f: 16,
            g: 17,
            h: 18
        }
    );

    binding(19, 20, 21.5);
    assignment(22, 23, 24.5);
    function_call(25, 26, 27.5);
    identifier(28, 29, 30.5);
    return_expr(31, 32, 33.5);
    arithmetic_expr(34, 35, 36.5);
    if_expr(37, 38, 39.5);
    while_expr(40, 41, 42);
    loop_expr(43, 44, 45);
}
