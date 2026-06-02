package main

import "fmt"

// Speaker is the dispatched Go interface. Dog is instantiated and converted to the
// Speaker interface in main, so the Go RTA driver resolves the interface invoke
// `s.Speak()` precisely to (Dog).Speak — the Go-edge half of the polyglot canary.
type Speaker interface {
	Speak() string
}

type Dog struct{}

func (Dog) Speak() string {
	return "woof"
}

// main is the Go root. Its single dynamic dispatch on the instantiated Dog is the Go
// RTA-resolved edge the canary asserts; the TS half (tokens.ts) is analyzed in the
// same run and must remain unaffected (no cross-language interference).
func main() {
	var s Speaker = Dog{}
	fmt.Println(s.Speak())
}
