use adopt_rust_fixture::greeting;

#[test]
fn greeting_returns_message() {
    assert_eq!(greeting("Ada"), "Hello, Ada");
}
