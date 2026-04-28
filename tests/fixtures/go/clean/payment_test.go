package payment

import "testing"

func TestAuthorizeDenied(t *testing.T) {
	err := Authorize(false)
	if err == nil {
		t.Fatal("expected denied error")
	}
}
