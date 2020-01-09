#![feature(negative_impls)]
#![feature(specialization)]

struct S;
struct Z;

default impl S {} //~ ERROR inherent impls cannot be `default`

default unsafe impl Send for S {} //~ ERROR impls of auto traits cannot be default
default impl !Send for Z {} //~ ERROR impls of auto traits cannot be default
                            //~^ ERROR negative impls cannot be default impls

trait Tr {}
default impl !Tr for S {} //~ ERROR negative impls cannot be default impls

fn main() {}
