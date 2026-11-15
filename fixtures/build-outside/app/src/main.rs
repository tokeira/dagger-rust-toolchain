//! A workspace that depends by path on a package outside its root. The
//! package reaches its commands through the includeExtraFiles setting.

fn main() {
    println!("{}", outside_shared::origin());
}
