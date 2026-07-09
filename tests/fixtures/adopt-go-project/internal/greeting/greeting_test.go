package greeting

import "testing"

func TestMessage(t *testing.T) {
	if got := Message("Ada"); got != "Hello, Ada" {
		t.Fatalf("Message() = %q", got)
	}
}
