// This example repo has one local polint rule: local/go-cyclomatic-complexity.
// The rule reports Go functions whose branch count exceeds the configured max.
// This tiny router intentionally has two if statements while .polint.toml sets
// max = 1, so the example stays small but still produces a diagnostic.
package router

func Route(kind string, ready bool) string {
	if kind == "admin" {
		// This branch contributes to the function's cyclomatic complexity.
		return "admin"
	}
	if ready {
		// This second branch pushes the function over the example threshold.
		return "ready"
	}
	return "guest"
}
