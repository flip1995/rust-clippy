#[no_mangle]
pub extern fn overflow() {
    let xs = [0, 1, 2, 3];
    let _y = unsafe { *xs.as_ptr().offset(4) };
}
