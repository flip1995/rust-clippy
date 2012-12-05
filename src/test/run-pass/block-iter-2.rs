// xfail-fast
#[legacy_modes];

fn iter_vec<T>(v: ~[T], f: fn(T)) { for v.each |x| { f(*x); } }

fn main() {
    let v = ~[1, 2, 3, 4, 5];
    let mut sum = 0;
    iter_vec(copy v, |i| {
        iter_vec(copy v, |j| {
            log(error, i * j);
            sum += i * j;
        });
    });
    log(error, sum);
    assert (sum == 225);
}
