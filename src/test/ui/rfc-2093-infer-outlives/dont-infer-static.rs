// ignore-tidy-linelength

/*
 * We don't infer `T: 'static` outlives relationships by default.
 * Instead an additional feature gate `infer_static_outlives_requirements`
 * is required.
 */

struct Foo<U> {
    bar: Bar<U> //~ ERROR the parameter type `U` may not live long enough [E0310]
}
struct Bar<T: 'static> {
    x: T,
}

fn main() {}
