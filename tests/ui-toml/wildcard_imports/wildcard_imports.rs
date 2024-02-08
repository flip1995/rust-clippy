#![warn(clippy::wildcard_imports)]

mod prelude {
    pub const FOO: u8 = 1;
}

mod utils {
    pub const BAR: u8 = 1;
    pub fn print() {}
}

mod my_crate {
    pub mod utils {
        pub fn my_util_fn() {}
    }
}

use utils::*;
//~^ ERROR: usage of wildcard import
use my_crate::utils::*;
//~^ ERROR: usage of wildcard import
use prelude::*;
//~^ ERROR: usage of wildcard import

fn main() {
    let _ = FOO;
    let _ = BAR;
    print();
    my_util_fn();
}
