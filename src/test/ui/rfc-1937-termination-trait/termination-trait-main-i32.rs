fn main() -> i32 {
//~^ ERROR `main` has invalid return type `i32`
//~| NOTE `main` can only return types that implement `std::process::Termination`
//~| HELP consider using `()`, or a `Result`
    0
}
