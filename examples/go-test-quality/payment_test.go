// This example repo has one local polint rule: local/go-test-quality.
// The rule heuristically flags Go tests that are too large or have no obvious
// assertion/error check. This test intentionally calls code without checking an
// outcome.
package payments

import "testing"

func TestAuthorize(t *testing.T) {
	// Intentional violation: this call has no assertion, error check, or t.Fatal.
	Authorize()
}

func Authorize() {}
