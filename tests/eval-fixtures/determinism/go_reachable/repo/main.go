package main

import "fmt"

// reachable is called directly from main, so the call site main -> reachable is
// inside the reachable-from-roots graph (in_reachable_graph = true, root-seed).
func reachable() {
	fmt.Println("reachable")
}

// orphanHelper is only ever called from orphan, which itself is never reached
// from any root, so its call site is outside the reachable graph.
func orphanHelper() {
	fmt.Println("orphan helper")
}

// orphan is never called from main (or any other root). The call site
// orphan -> orphanHelper is therefore OUTSIDE the reachable graph
// (in_reachable_graph = false) — the unreachable mark the Phase 43 determinism
// gate asserts each fixture produces.
func orphan() {
	orphanHelper()
}

// main is the Go root (RootKind::Main). It directly calls reachable().
func main() {
	reachable()
}
