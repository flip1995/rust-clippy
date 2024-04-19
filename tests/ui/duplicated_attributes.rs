//@aux-build:proc_macro_attr.rs

#![warn(clippy::duplicated_attributes)]
#![cfg(any(unix, windows))]
#![allow(dead_code)]
#![allow(dead_code)] //~ ERROR: duplicated attribute
#![cfg(any(unix, windows))] // Should not warn!

#[macro_use]
extern crate proc_macro_attr;

#[cfg(any(unix, windows, target_os = "linux"))]
#[allow(dead_code)]
#[allow(dead_code)] //~ ERROR: duplicated attribute
#[cfg(any(unix, windows, target_os = "linux"))] // Should not warn!
fn foo() {}

#[cfg(unix)]
#[cfg(windows)]
#[cfg(unix)] // cfgs are not handled
fn bar() {}

#[proc_macro_attr::duplicated_attr()] // Should not warn!
fn babar() {}

fn main() {}
