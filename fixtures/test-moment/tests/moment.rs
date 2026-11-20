//! A test that prints more lines than a check's message keeps, then fails
//! with the moment it ran at.

#[test]
fn fails_at_a_new_moment() {
    for line in 0..250 {
        println!("line {line} of 250");
    }
    panic!("ran-at-{}", test_moment::moment());
}
