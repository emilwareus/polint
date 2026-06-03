package main

import "fmt"

// Greeter is the dispatched interface. Dog is instantiated and converted to Greeter in
// main, so the Go RTA driver derives the main -> (Dog).Greet interface edge (the solver
// derivation whose normalized observed JSON the determinism gate shuffles).
type Greeter interface {
	Greet() string
}

type Dog struct{}

func (Dog) Greet() string {
	return "woof"
}

// orphanHelper is only called from orphan, which is never reached from any root, so its
// call site is OUTSIDE the reachable graph (in_reachable_graph = false) — the
// unreachable mark the Phase 43 determinism gate's marking invariant asserts.
func orphanHelper() {
	fmt.Println("orphan helper")
}

// orphan is unexported and never called from main (or any root), so it is unreachable.
func orphan() {
	orphanHelper()
}

// main is the Go root (RootKind::Main). It instantiates Dog, converts it to Greeter,
// and performs the dynamic dispatch the RTA driver resolves.
func main() {
	var g Greeter = Dog{}
	fmt.Println(g.Greet())
}
