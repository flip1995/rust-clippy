#![feature(optin_builtin_traits)]
// edition:2018

// This tests the the specialized async-await-specific error when futures don't implement an
// auto trait (which is specifically Send) due to some type that was captured.

struct Foo;

impl !Send for Foo {}

fn is_send<T: Send>(t: T) { }

async fn bar() {
    let x = Foo;
    baz().await;
}

async fn baz() { }

fn main() {
    is_send(bar());
    //~^ ERROR future cannot be sent between threads safely
}
