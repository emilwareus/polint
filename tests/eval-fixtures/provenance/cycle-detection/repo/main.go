// Cycle-detection fixture (D-11/D-12).
//
// `ping` and `pong` are mutually recursive: ping -> pong -> ping. Each call site
// also threads a value through the return (a place-to-place value copy), so the
// projected semantic graph contains a value-flow / call cycle that passes through
// the call (summary) constraints.
//
// This is the native-tree counterpart to the solver/validate unit cycle test: it
// proves the solver does NOT diverge on a solver -> summary -> solver constraint
// set. Summaries are an INPUT snapshot to the solver, never re-fed into the same
// fixpoint as they are produced (D-12), and the SolverBudget bounds the outer
// iterations (D-11). The run therefore terminates within the runtime budget rather
// than looping unbounded — the assertion below.
package main

import "fmt"

// ping calls pong (ping -> pong call site + CallConstraint) and copies the result
// into a local (CopyEdge), forming one half of the cycle.
func ping(depth int) int {
	if depth <= 0 {
		return 0
	}
	next := pong(depth - 1)
	echo := next
	return echo
}

// pong calls ping (pong -> ping call site), closing the ping <-> pong cycle.
func pong(depth int) int {
	if depth <= 0 {
		return 0
	}
	prev := ping(depth - 1)
	echo := prev
	return echo
}

func main() {
	result := ping(3)
	fmt.Println(result)
}
