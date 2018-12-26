// ignore-tidy-linelength
// revisions: ast mir
//[mir]compile-flags: -Z borrowck=mir

fn inside_closure(x: &mut i32) {
}

fn outside_closure_1(x: &mut i32) {
}

fn outside_closure_2(x: &i32) {
}

fn foo(a: &mut i32) {
    let bar = || {
        inside_closure(a)
    };
    outside_closure_1(a); //[ast]~ ERROR cannot borrow `*a` as mutable because previous closure requires unique access
    //[mir]~^ ERROR cannot borrow `*a` as mutable because previous closure requires unique access

    outside_closure_2(a); //[ast]~ ERROR cannot borrow `*a` as immutable because previous closure requires unique access
    //[mir]~^ ERROR cannot borrow `*a` as immutable because previous closure requires unique access

    drop(bar);
}

fn main() {
}
