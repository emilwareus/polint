// This example repo has one local polint rule: local/go-import-boundaries.
// The rule reads forbidden import boundaries from .polint.toml. This package is
// intentionally configured so importing net/http is not allowed.
package billing

// Intentional violation: .polint.toml forbids this import for handler.go.
import "net/http"

func Method() string {
	return http.MethodPost
}
