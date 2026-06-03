package main

import "fmt"

// Speaker is the dispatched interface.
type Speaker interface {
	Speak() string
}

// Dog is INSTANTIATED in the reachable program (Dog{} is converted to the Speaker
// interface inside runDispatch), so RTA's instantiated-type set contains Dog and the
// interface invoke `s.Speak()` resolves precisely to (Dog).Speak.
type Dog struct{}

func (Dog) Speak() string {
	return "woof"
}

// runDispatch is an UNEXPORTED helper that is NOT a reachability root. main reaches it
// only via a DIRECT (static) call. The interface dispatch `s.Speak()` lives HERE, not
// in main. RTA must grow reachability over the static main -> runDispatch edge so this
// dispatch is resolved; if reachability only grew along dynamic edges, runDispatch would
// never enter the worklist and `runDispatch -> (Dog).Speak` would be silently dropped.
func runDispatch() {
	var s Speaker = Dog{}
	fmt.Println(s.Speak())
}

// main is the Go root. It makes a single DIRECT call to the unexported runDispatch.
func main() {
	runDispatch()
}
