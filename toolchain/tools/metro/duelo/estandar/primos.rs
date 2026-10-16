use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let n: i64 = s.trim().parse().unwrap();
    let mut c = 0;
    for k in 2..n {
        let mut p = 1;
        let mut d = 2;
        while d * d <= k {
            if k % d == 0 { p = 0; break; }
            d += 1;
        }
        c += p;
    }
    println!("{c}");
}
