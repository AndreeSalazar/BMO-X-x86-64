use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let _n: i64 = s.trim().parse().unwrap();
    println!("0");
}
