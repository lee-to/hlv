from adopt_demo import greeting


def test_greeting_returns_message() -> None:
    assert greeting("Ada") == "Hello, Ada"
