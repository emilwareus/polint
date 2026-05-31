package main

import "fmt"

// greet returns a greeting; called directly from main so the call site
// main -> greet projects a Call edge + CallConstraint into the semantic graph.
func greet(name string) string {
	return "hello " + name
}

// main is the Go root. It performs a direct call (greet) and a value copy
// (message := greet(...)), exercising the Call edge, CallConstraint, and the
// place-to-place CopyEdge constraint the minimal projection emits.
func main() {
	message := greet("world")
	other := message
	fmt.Println(other)
}
