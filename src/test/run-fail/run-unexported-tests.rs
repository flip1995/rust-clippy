// error-pattern:ran an unexported test
// compile-flags:--test
// check-stdout

mod m {
    pub fn exported() {}

    #[test]
    fn unexported() {
        panic!("ran an unexported test");
    }
}
