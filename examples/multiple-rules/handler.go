// This example repo has two local polint rules registered by one Cargo crate.
// local/go-import-boundaries catches this forbidden Go import.
package billing

// Intentional violation: .polint.toml forbids this import for Go files.
import "net/http"

func Method() string {
	return http.MethodPost
}
