use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let n: usize = s.trim().parse().unwrap();
    let mut a: Vec<i64> = (0..n as i64).map(|i| (i * 7919) % 1000).collect();
    for i in 1..n {
        let v = a[i];
        let mut j = i as i64 - 1;
        while j >= 0 && a[j as usize] > v { a[j as usize + 1] = a[j as usize]; j -= 1; }
        a[(j + 1) as usize] = v;
    }
    println!("{} {}", a[0], a[n - 1]);
}
