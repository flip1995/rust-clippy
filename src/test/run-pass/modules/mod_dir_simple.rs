// run-pass
// ignore-pretty issue #37195

mod mod_dir_simple {
    pub mod test;
}

pub fn main() {
    assert_eq!(mod_dir_simple::test::foo(), 10);
}
