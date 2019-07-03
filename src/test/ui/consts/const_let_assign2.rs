// build-pass (FIXME(62277): could be check-pass?)

pub struct AA {
    pub data: [u8; 10],
}

impl AA {
    pub const fn new() -> Self {
        let mut res: AA = AA { data: [0; 10] };
        res.data[0] = 5;
        res
    }
}

static mut BB: AA = AA::new();

fn main() {
    let ptr = unsafe { &mut BB };
    for a in ptr.data.iter() {
        println!("{}", a);
    }
}
