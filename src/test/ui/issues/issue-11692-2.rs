fn main() {
    concat!(test!()); //~ ERROR cannot find macro `test!` in this scope
                      //~| ERROR expected a literal
}
