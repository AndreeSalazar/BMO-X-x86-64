use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let n: i64 = s.trim().parse().unwrap();
    let mut a: Vec<u8> = Vec::new();
    for i in 0..n { a.push((i * 7) as u8); }
    let mut t: i64 = 0;
    for b in &a { t += *b as i64; }
    println!("{t}");
}
