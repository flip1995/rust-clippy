#![allow(
    dead_code,
    clippy::missing_safety_doc,
    clippy::extra_unused_lifetimes,
    clippy::extra_unused_type_parameters
)]
#![warn(clippy::new_without_default)]

pub struct Foo;

impl Foo {
    pub fn new() -> Foo {
        //~^ ERROR: you should consider adding a `Default` implementation for `Foo`
        //~| NOTE: `-D clippy::new-without-default` implied by `-D warnings`
        Foo
    }
}

pub struct Bar;

impl Bar {
    pub fn new() -> Self {
        //~^ ERROR: you should consider adding a `Default` implementation for `Bar`
        Bar
    }
}

pub struct Ok;

impl Ok {
    pub fn new() -> Self {
        Ok
    }
}

impl Default for Ok {
    fn default() -> Self {
        Ok
    }
}

pub struct Params;

impl Params {
    pub fn new(_: u32) -> Self {
        Params
    }
}

pub struct GenericsOk<T> {
    bar: T,
}

impl<U> Default for GenericsOk<U> {
    fn default() -> Self {
        unimplemented!();
    }
}

impl<'c, V> GenericsOk<V> {
    pub fn new() -> GenericsOk<V> {
        unimplemented!()
    }
}

pub struct LtOk<'a> {
    foo: &'a bool,
}

impl<'b> Default for LtOk<'b> {
    fn default() -> Self {
        unimplemented!();
    }
}

impl<'c> LtOk<'c> {
    pub fn new() -> LtOk<'c> {
        unimplemented!()
    }
}

pub struct LtKo<'a> {
    foo: &'a bool,
}

impl<'c> LtKo<'c> {
    pub fn new() -> LtKo<'c> {
        //~^ ERROR: you should consider adding a `Default` implementation for `LtKo<'c>`
        unimplemented!()
    }
}

struct Private;

impl Private {
    fn new() -> Private {
        unimplemented!()
    } // We don't lint private items
}

struct PrivateStruct;

impl PrivateStruct {
    pub fn new() -> PrivateStruct {
        unimplemented!()
    } // We don't lint public items on private structs
}

pub struct PrivateItem;

impl PrivateItem {
    fn new() -> PrivateItem {
        unimplemented!()
    } // We don't lint private items on public structs
}

struct Const;

impl Const {
    pub const fn new() -> Const {
        Const
    } // const fns can't be implemented via Default
}

pub struct IgnoreGenericNew;

impl IgnoreGenericNew {
    pub fn new<T>() -> Self {
        IgnoreGenericNew
    } // the derived Default does not make sense here as the result depends on T
}

pub trait TraitWithNew: Sized {
    fn new() -> Self {
        panic!()
    }
}

pub struct IgnoreUnsafeNew;

impl IgnoreUnsafeNew {
    pub unsafe fn new() -> Self {
        IgnoreUnsafeNew
    }
}

#[derive(Default)]
pub struct OptionRefWrapper<'a, T>(Option<&'a T>);

impl<'a, T> OptionRefWrapper<'a, T> {
    pub fn new() -> Self {
        OptionRefWrapper(None)
    }
}

pub struct Allow(Foo);

impl Allow {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        unimplemented!()
    }
}

pub struct AllowDerive;

impl AllowDerive {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        unimplemented!()
    }
}

pub struct NewNotEqualToDerive {
    foo: i32,
}

impl NewNotEqualToDerive {
    // This `new` implementation is not equal to a derived `Default`, so do not suggest deriving.
    pub fn new() -> Self {
        //~^ ERROR: you should consider adding a `Default` implementation for `NewNotEqualToDe
        NewNotEqualToDerive { foo: 1 }
    }
}

// see #6933
pub struct FooGenerics<T>(std::marker::PhantomData<T>);
impl<T> FooGenerics<T> {
    pub fn new() -> Self {
        //~^ ERROR: you should consider adding a `Default` implementation for `FooGenerics<T>`
        Self(Default::default())
    }
}

pub struct BarGenerics<T>(std::marker::PhantomData<T>);
impl<T: Copy> BarGenerics<T> {
    pub fn new() -> Self {
        //~^ ERROR: you should consider adding a `Default` implementation for `BarGenerics<T>`
        Self(Default::default())
    }
}

pub mod issue7220 {
    pub struct Foo<T> {
        _bar: *mut T,
    }

    impl<T> Foo<T> {
        pub fn new() -> Self {
            //~^ ERROR: you should consider adding a `Default` implementation for `Foo<T>`
            todo!()
        }
    }
}

// see issue #8152
// This should not create any lints
pub struct DocHidden;
impl DocHidden {
    #[doc(hidden)]
    pub fn new() -> Self {
        DocHidden
    }
}

fn main() {}

pub struct IgnoreConstGenericNew(usize);
impl IgnoreConstGenericNew {
    pub fn new<const N: usize>() -> Self {
        Self(N)
    }
}

pub struct IgnoreLifetimeNew;
impl IgnoreLifetimeNew {
    pub fn new<'a>() -> Self {
        Self
    }
}

// From issue #11267

pub struct MyStruct<K, V>
where
    K: std::hash::Hash + Eq + PartialEq,
{
    _kv: Option<(K, V)>,
}

impl<K, V> MyStruct<K, V>
where
    K: std::hash::Hash + Eq + PartialEq,
{
    pub fn new() -> Self {
        Self { _kv: None }
    }
}
