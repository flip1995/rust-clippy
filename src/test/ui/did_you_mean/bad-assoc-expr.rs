fn main() {
    let a = [1, 2, 3, 4];
    [i32; 4]::clone(&a);
    //~^ ERROR missing angle brackets in associated item path

    [i32]::as_ref(&a);
    //~^ ERROR missing angle brackets in associated item path

    (u8)::clone(&0);
    //~^ ERROR missing angle brackets in associated item path

    (u8, u8)::clone(&(0, 0));
    //~^ ERROR missing angle brackets in associated item path

    &(u8)::clone(&0);
    //~^ ERROR missing angle brackets in associated item path

    10 + (u8)::clone(&0);
    //~^ ERROR missing angle brackets in associated item path
}
