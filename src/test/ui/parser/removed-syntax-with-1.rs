fn main() {
    struct S {
        foo: (),
        bar: (),
    }

    let a = S { foo: (), bar: () };
    let b = S { foo: () with a };
    //~^ ERROR expected one of `,`, `.`, `?`, `}`, or an operator, found `with`
    //~| ERROR missing field `bar` in initializer of `main::S`
}
