fn main() {
    let started = std::time::Instant::now();
    let answer = kinds_user::answer();
    println!("{answer} in {:?}", started.elapsed());
}
