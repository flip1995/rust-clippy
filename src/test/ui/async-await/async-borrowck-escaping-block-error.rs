// edition:2018
// run-rustfix

fn foo() -> Box<impl std::future::Future<Output = u32>> {
    let x = 0u32;
    Box::new(async { x } )
    //~^ ERROR E0373
}

fn main() {
    let _foo = foo();
}
