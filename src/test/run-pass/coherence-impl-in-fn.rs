fn main() {
    enum x { foo }
    impl x : core::cmp::Eq {
        pure fn eq(&&other: x) -> bool { self as int == other as int }
    }
}
