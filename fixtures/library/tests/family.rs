#[test]
fn names_a_family_from_outside() {
    assert!(!library::family().is_empty());
}
