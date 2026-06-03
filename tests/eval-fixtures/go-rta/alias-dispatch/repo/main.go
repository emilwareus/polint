package main

import "fmt"

// Speaker is the dispatched interface.
type Speaker interface {
	Speak() string
}

// Dog is the concrete implementer. Its method receiver is `Dog` (the underlying type).
type Dog struct{}

func (Dog) Speak() string {
	return "woof"
}

// AliasDog is a type ALIAS for Dog (not a distinct defined type). go/types reports a
// value `AliasDog{}` converted to an interface, and AliasDog's package-scope method-set,
// under the ALIAS spelling `...AliasDog`, while (Dog).Speak's receiver is the UNDERLYING
// `...Dog`. The sidecar canonicalizes the instantiated_type and the method_set key through
// the alias (types.Unalias) so all three share the underlying `...Dog` identity and the
// interface invoke `s.Speak()` resolves to (Dog).Speak instead of being silently dropped.
type AliasDog = Dog

// Cat also implements Speaker (in its method-set) but is NEVER instantiated anywhere
// reachable — the RTA instantiated-type filter must EXCLUDE (Cat).Speak even though the
// value flowing through the interface is spelled via an alias. This keeps the alias proof
// honest: the edge resolves because Dog is (canonically) instantiated, not because the
// resolver fell back to coarse CHA over every Speaker implementer.
type Cat struct{}

func (Cat) Speak() string {
	return "meow"
}

// main is the Go root. It instantiates exactly AliasDog (== Dog), converts it to Speaker,
// and performs one dynamic dispatch. RTA must resolve main -> (Dog).Speak and nothing else.
func main() {
	var s Speaker = AliasDog{}
	fmt.Println(s.Speak())
}
