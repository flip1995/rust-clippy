#![feature(reverse_bits)]

fn main() {
    let x: &'static i32 = &(5_i32.reverse_bits());
        //~^ ERROR does not live long enough
    let y: &'static i32 = &(i32::from_be_bytes([0x12, 0x34, 0x56, 0x78]));
        //~^ ERROR does not live long enough
    let z: &'static i32 = &(i32::from_le_bytes([0x12, 0x34, 0x56, 0x78]));
        //~^ ERROR does not live long enough
    let a: &'static i32 = &(i32::from_be(i32::from_ne_bytes([0x80, 0, 0, 0])));
        //~^ ERROR does not live long enough
    let b: &'static [u8] = &(0x12_34_56_78_i32.to_be_bytes());
        //~^ ERROR does not live long enough
    let c: &'static [u8] = &(0x12_34_56_78_i32.to_le_bytes());
        //~^ ERROR does not live long enough
    let d: &'static [u8] = &(i32::min_value().to_be().to_ne_bytes());
        //~^ ERROR does not live long enough
}
