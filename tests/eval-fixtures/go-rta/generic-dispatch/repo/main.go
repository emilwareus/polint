package main

import "fmt"

// Speaker is the dispatched interface. It is satisfied by a method declared on a
// GENERIC type, instantiated to a concrete type argument in the reachable program.
type Speaker interface {
	Speak() string
}

// Box is a GENERIC type. Its method Speak satisfies Speaker. The x/tools RTA harvest
// keys a method-set by the type's `*types.Named` identity: the SSA program records the
// INSTANTIATED named type `Box[int]` in its runtime-type set with a real method value
// `(Box[int]).Speak`, while the syntactic declaration is `Box[T any]`. The frontend must
// emit the method-set + concrete method keyed by the INSTANTIATED identity `Box[int]`,
// or `method_sets.get("Box[int]")` misses and the interface invoke loses the edge
// Dispatch through generic types must not silently under-resolve.
type Box[T any] struct{ v T }

func (b Box[T]) Speak() string {
	return fmt.Sprint(b.v)
}

// main is the Go root. It instantiates exactly Box[int], converts it to Speaker, and
// performs one dynamic dispatch. RTA must resolve main -> (Box[int]).Speak: Box[int] is
// the instantiated rapid-type, and the generic method's method-set must be keyed by that
// instantiated identity so the invoke of `Speak` resolves.
func main() {
	var s Speaker = Box[int]{v: 1}
	fmt.Println(s.Speak())
}
