package main

import "fmt"

// Speaker is the dispatched interface, satisfied by a method on a GENERIC type
// instantiated to Box[int].
type Speaker interface {
	Speak() string
}

// Box is a GENERIC type whose method Speak satisfies Speaker.
type Box[T any] struct{ v T }

func (b Box[T]) Speak() string {
	return fmt.Sprint(b.v)
}

// IntBox is a SAME-PACKAGE type ALIAS for the generic INSTANTIATION Box[int]. Two
// independent method_set emitters in the sidecar would otherwise BOTH emit a method_set
// keyed by the canonical instantiated identity `...Box[int]`:
//   - emitMethodSets walks package scope, finds the alias TypeName IntBox, and (since
//     FIX-05) canonicalizes it through types.Unalias to `...Box[int]`.
//   - emitInstantiatedMethodSets harvests the reachable RuntimeTypes() instantiation
//     Box[int] (from useDirect's direct conversion below).
// With SEPARATE per-function `seen` maps both rows survived → a duplicate non-empty
// method_set stable_key → `validate_unique("method_set", ...)` rejected the WHOLE Go fact
// set → RTA derived ZERO edges repo-wide (review #4 catastrophic regression). The fix
// coordinates the two emitters to keep exactly ONE method_set row per canonical stable_key,
// so the Go facts are non-empty and the dispatch resolves.
type IntBox = Box[int]

// useDirect makes Box[int] reachable via its NON-alias spelling, so the SSA RuntimeTypes()
// set records the instantiation and emitInstantiatedMethodSets fires on Box[int] — the
// other half of the alias↔instantiation collision.
func useDirect() Speaker { return Box[int]{v: 2} }

// main is the Go root. It converts the alias-spelled IntBox AND (via useDirect) the
// direct Box[int] to Speaker and dispatches. RTA must resolve the invoke of `Speak` to the
// instantiated (Box[int]).Speak.
func main() {
	var s Speaker = IntBox{v: 1}
	fmt.Println(s.Speak())
	fmt.Println(useDirect().Speak())
}
