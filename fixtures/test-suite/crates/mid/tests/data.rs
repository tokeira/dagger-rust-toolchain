//! Runs in the package's directory, as Cargo runs it, and reads test data
//! from there.

#[test]
fn reads_its_own_data() {
    let expected = std::fs::read_to_string("tests/data/suffix.txt").expect("the package's test data");
    assert!(suite_mid::phrase().ends_with(expected.trim()));
}
