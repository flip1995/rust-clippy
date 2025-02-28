#![warn(clippy::io_other_error)]
use std::fmt;

#[derive(Debug)]
struct E;

impl std::error::Error for E {}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("E")
    }
}

macro_rules! o {
    {} => { std::io::ErrorKind::Other };
}

macro_rules! e {
    { $kind:expr } => { std::io::Error::new($kind, E) };
}

fn main() {
    let _err = std::io::Error::new(std::io::ErrorKind::Other, E);
    //~^ ERROR: this can be `std::io::Error::other(_)`
    let other = std::io::ErrorKind::Other;
    let _err = std::io::Error::new(other, E);
    //~^ ERROR: this can be `std::io::Error::other(_)`

    // not other
    let _err = std::io::Error::new(std::io::ErrorKind::TimedOut, E);

    // from expansion
    let _err = e!(other);
    let _err = std::io::Error::new(o!(), E);
    let _err = e!(o!());

    paths::short();
    under_msrv();
}

mod paths {
    use std::io::{self, Error, ErrorKind};

    pub fn short() {
        let _err = Error::new(ErrorKind::Other, super::E);
        //~^ ERROR: this can be `std::io::Error::other(_)`
        let _err = io::Error::new(io::ErrorKind::Other, super::E);
        //~^ ERROR: this can be `std::io::Error::other(_)`
    }
}

#[clippy::msrv = "1.73"]
fn under_msrv() {
    let _err = std::io::Error::new(std::io::ErrorKind::Other, E);
}
