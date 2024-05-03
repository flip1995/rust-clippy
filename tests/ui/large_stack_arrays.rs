//@aux-build:proc_macros.rs
#![warn(clippy::large_stack_arrays)]
#![allow(clippy::large_enum_variant)]

extern crate proc_macros;

#[derive(Clone, Copy)]
struct S {
    pub data: [u64; 32],
}

#[derive(Clone, Copy)]
enum E {
    S(S),
    T(u32),
}

pub static DOESNOTLINT: [u8; 512_001] = [0; 512_001];
pub static DOESNOTLINT2: [u8; 512_001] = {
    let x = 0;
    [x; 512_001]
};

fn issue_10741() {
    #[derive(Copy, Clone)]
    struct Large([u32; 100_000]);

    fn build() -> Large {
        Large([0; 100_000])
    }

    let _x = [build(); 3];
    //~^ ERROR: allocating a local array larger than 512000 bytes

    let _y = [build(), build(), build()];
    //~^ ERROR: allocating a local array larger than 512000 bytes
}

fn main() {
    let bad = (
        [0u32; 20_000_000],
        //~^ ERROR: allocating a local array larger than 512000 bytes
        [S { data: [0; 32] }; 5000],
        //~^ ERROR: allocating a local array larger than 512000 bytes
        [Some(""); 20_000_000],
        //~^ ERROR: allocating a local array larger than 512000 bytes
        [E::T(0); 5000],
        //~^ ERROR: allocating a local array larger than 512000 bytes
        [0u8; usize::MAX],
        //~^ ERROR: allocating a local array larger than 512000 bytes
    );

    let good = (
        [0u32; 1000],
        [S { data: [0; 32] }; 1000],
        [Some(""); 1000],
        [E::T(0); 1000],
        [(); 20_000_000],
    );
}

#[allow(clippy::useless_vec)]
fn issue_12586() {
    macro_rules! dummy {
        ($n:expr) => {
            $n
        };
        // Weird rule to test help messages.
        ($a:expr => $b:expr) => {
            [$a, $b, $a, $b]
            //~^ ERROR: allocating a local array larger than 512000 bytes
        };
        ($id:ident; $n:literal) => {
            dummy!(::std::vec![$id;$n])
        };
        ($($id:expr),+ $(,)?) => {
            ::std::vec![$($id),*]
        }
    }
    macro_rules! create_then_move {
        ($id:ident; $n:literal) => {{
            let _x_ = [$id; $n];
            //~^ ERROR: allocating a local array larger than 512000 bytes
            _x_
        }};
    }

    let x = [0u32; 50_000];
    let y = vec![x, x, x, x, x];
    let y = vec![dummy![x, x, x, x, x]];
    let y = vec![dummy![[x, x, x, x, x]]];
    let y = dummy![x, x, x, x, x];
    let y = [x, x, dummy!(x), x, x];
    //~^ ERROR: allocating a local array larger than 512000 bytes
    let y = dummy![x => x];
    let y = dummy![x;5];
    let y = dummy!(vec![dummy![x, x, x, x, x]]);
    let y = dummy![[x, x, x, x, x]];
    //~^ ERROR: allocating a local array larger than 512000 bytes

    let y = proc_macros::make_it_big!([x; 1]);
    //~^ ERROR: allocating a local array larger than 512000 bytes
    let y = vec![proc_macros::make_it_big!([x; 10])];
    let y = vec![create_then_move![x; 5]; 5];
}
