// aux-build:arc_wake.rs
// edition:2018
// run-pass

// Test that the drop order for parameters in a fn and async fn matches up. Also test that
// parameters (used or unused) are not dropped until the async fn is cancelled.
// This file is mostly copy-pasted from drop-order-for-async-fn-parameters.rs

extern crate arc_wake;

use arc_wake::ArcWake;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};

struct EmptyWaker;

impl ArcWake for EmptyWaker {
    fn wake(self: Arc<Self>) {}
}

#[derive(Debug, Eq, PartialEq)]
enum DropOrder {
    Function,
    Val(&'static str),
}

type DropOrderListPtr = Rc<RefCell<Vec<DropOrder>>>;

struct D(&'static str, DropOrderListPtr);

impl Drop for D {
    fn drop(&mut self) {
        self.1.borrow_mut().push(DropOrder::Val(self.0));
    }
}

struct NeverReady;

impl Future for NeverReady {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Pending
    }
}

/// Check that unused bindings are dropped after the function is polled.
async fn foo_async(x: D, _y: D) {
    x.1.borrow_mut().push(DropOrder::Function);
    NeverReady.await;
}

fn foo_sync(x: D, _y: D) {
    x.1.borrow_mut().push(DropOrder::Function);
}

/// Check that underscore patterns are dropped after the function is polled.
async fn bar_async(x: D, _: D) {
    x.1.borrow_mut().push(DropOrder::Function);
    NeverReady.await;
}

fn bar_sync(x: D, _: D) {
    x.1.borrow_mut().push(DropOrder::Function);
}

/// Check that underscore patterns within more complex patterns are dropped after the function
/// is polled.
async fn baz_async((x, _): (D, D)) {
    x.1.borrow_mut().push(DropOrder::Function);
    NeverReady.await;
}

fn baz_sync((x, _): (D, D)) {
    x.1.borrow_mut().push(DropOrder::Function);
}

/// Check that underscore and unused bindings within and outwith more complex patterns are dropped
/// after the function is polled.
async fn foobar_async(x: D, (a, _, _c): (D, D, D), _: D, _y: D) {
    x.1.borrow_mut().push(DropOrder::Function);
    NeverReady.await;
}

fn foobar_sync(x: D, (a, _, _c): (D, D, D), _: D, _y: D) {
    x.1.borrow_mut().push(DropOrder::Function);
}

struct Foo;

impl Foo {
    /// Check that unused bindings are dropped after the method is polled.
    async fn foo_async(x: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn foo_sync(x: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
    }

    /// Check that underscore patterns are dropped after the method is polled.
    async fn bar_async(x: D, _: D) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn bar_sync(x: D, _: D) {
        x.1.borrow_mut().push(DropOrder::Function);
    }

    /// Check that underscore patterns within more complex patterns are dropped after the method
    /// is polled.
    async fn baz_async((x, _): (D, D)) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn baz_sync((x, _): (D, D)) {
        x.1.borrow_mut().push(DropOrder::Function);
    }

    /// Check that underscore and unused bindings within and outwith more complex patterns are
    /// dropped after the method is polled.
    async fn foobar_async(x: D, (a, _, _c): (D, D, D), _: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn foobar_sync(x: D, (a, _, _c): (D, D, D), _: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
    }
}

struct Bar<'a>(PhantomData<&'a ()>);

impl<'a> Bar<'a> {
    /// Check that unused bindings are dropped after the method with self is polled.
    async fn foo_async(&'a self, x: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn foo_sync(&'a self, x: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
    }

    /// Check that underscore patterns are dropped after the method with self is polled.
    async fn bar_async(&'a self, x: D, _: D) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn bar_sync(&'a self, x: D, _: D) {
        x.1.borrow_mut().push(DropOrder::Function);
    }

    /// Check that underscore patterns within more complex patterns are dropped after the method
    /// with self is polled.
    async fn baz_async(&'a self, (x, _): (D, D)) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn baz_sync(&'a self, (x, _): (D, D)) {
        x.1.borrow_mut().push(DropOrder::Function);
    }

    /// Check that underscore and unused bindings within and outwith more complex patterns are
    /// dropped after the method with self is polled.
    async fn foobar_async(&'a self, x: D, (a, _, _c): (D, D, D), _: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
        NeverReady.await;
    }

    fn foobar_sync(&'a self, x: D, (a, _, _c): (D, D, D), _: D, _y: D) {
        x.1.borrow_mut().push(DropOrder::Function);
    }
}

fn assert_drop_order_after_cancel<Fut: Future<Output = ()>>(
    f: impl FnOnce(DropOrderListPtr) -> Fut,
    g: impl FnOnce(DropOrderListPtr),
) {
    let empty = Arc::new(EmptyWaker);
    let waker = ArcWake::into_waker(empty);
    let mut cx = Context::from_waker(&waker);

    let actual_order = Rc::new(RefCell::new(Vec::new()));
    let mut fut = Box::pin(f(actual_order.clone()));
    let _ = fut.as_mut().poll(&mut cx);

    // Parameters are never dropped until the future completes.
    assert_eq!(*actual_order.borrow(), vec![DropOrder::Function]);

    drop(fut);

    let expected_order = Rc::new(RefCell::new(Vec::new()));
    g(expected_order.clone());
    assert_eq!(*actual_order.borrow(), *expected_order.borrow());
}

fn main() {
    // Free functions (see doc comment on function for what it tests).
    assert_drop_order_after_cancel(
        |l| foo_async(D("x", l.clone()), D("_y", l.clone())),
        |l| foo_sync(D("x", l.clone()), D("_y", l.clone())),
    );
    assert_drop_order_after_cancel(
        |l| bar_async(D("x", l.clone()), D("_", l.clone())),
        |l| bar_sync(D("x", l.clone()), D("_", l.clone())),
    );
    assert_drop_order_after_cancel(
        |l| baz_async((D("x", l.clone()), D("_", l.clone()))),
        |l| baz_sync((D("x", l.clone()), D("_", l.clone()))),
    );
    assert_drop_order_after_cancel(
        |l| {
            foobar_async(
                D("x", l.clone()),
                (D("a", l.clone()), D("_", l.clone()), D("_c", l.clone())),
                D("_", l.clone()),
                D("_y", l.clone()),
            )
        },
        |l| {
            foobar_sync(
                D("x", l.clone()),
                (D("a", l.clone()), D("_", l.clone()), D("_c", l.clone())),
                D("_", l.clone()),
                D("_y", l.clone()),
            )
        },
    );

    // Methods w/out self (see doc comment on function for what it tests).
    assert_drop_order_after_cancel(
        |l| Foo::foo_async(D("x", l.clone()), D("_y", l.clone())),
        |l| Foo::foo_sync(D("x", l.clone()), D("_y", l.clone())),
    );
    assert_drop_order_after_cancel(
        |l| Foo::bar_async(D("x", l.clone()), D("_", l.clone())),
        |l| Foo::bar_sync(D("x", l.clone()), D("_", l.clone())),
    );
    assert_drop_order_after_cancel(
        |l| Foo::baz_async((D("x", l.clone()), D("_", l.clone()))),
        |l| Foo::baz_sync((D("x", l.clone()), D("_", l.clone()))),
    );
    assert_drop_order_after_cancel(
        |l| {
            Foo::foobar_async(
                D("x", l.clone()),
                (D("a", l.clone()), D("_", l.clone()), D("_c", l.clone())),
                D("_", l.clone()),
                D("_y", l.clone()),
            )
        },
        |l| {
            Foo::foobar_sync(
                D("x", l.clone()),
                (D("a", l.clone()), D("_", l.clone()), D("_c", l.clone())),
                D("_", l.clone()),
                D("_y", l.clone()),
            )
        },
    );

    // Methods (see doc comment on function for what it tests).
    let b = Bar(Default::default());
    assert_drop_order_after_cancel(
        |l| b.foo_async(D("x", l.clone()), D("_y", l.clone())),
        |l| b.foo_sync(D("x", l.clone()), D("_y", l.clone())),
    );
    assert_drop_order_after_cancel(
        |l| b.bar_async(D("x", l.clone()), D("_", l.clone())),
        |l| b.bar_sync(D("x", l.clone()), D("_", l.clone())),
    );
    assert_drop_order_after_cancel(
        |l| b.baz_async((D("x", l.clone()), D("_", l.clone()))),
        |l| b.baz_sync((D("x", l.clone()), D("_", l.clone()))),
    );
    assert_drop_order_after_cancel(
        |l| {
            b.foobar_async(
                D("x", l.clone()),
                (D("a", l.clone()), D("_", l.clone()), D("_c", l.clone())),
                D("_", l.clone()),
                D("_y", l.clone()),
            )
        },
        |l| {
            b.foobar_sync(
                D("x", l.clone()),
                (D("a", l.clone()), D("_", l.clone()), D("_c", l.clone())),
                D("_", l.clone()),
                D("_y", l.clone()),
            )
        },
    );
}
