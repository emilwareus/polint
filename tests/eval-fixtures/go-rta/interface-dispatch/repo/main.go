package main

import "fmt"

// Speaker is the dispatched interface.
type Speaker interface {
	Speak() string
}

// Dog is INSTANTIATED in the reachable program (Dog{} is converted to the Speaker
// interface in main), so RTA's instantiated-type set contains Dog and the
// interface invoke `s.Speak()` resolves precisely to (Dog).Speak.
type Dog struct{}

func (Dog) Speak() string {
	return "woof"
}

// Cat ALSO implements Speaker and declares Speak in its method-set, but Cat is
// NEVER instantiated/converted-to-interface anywhere reachable. RTA (unlike coarse
// CHA) must therefore EXCLUDE (Cat).Speak from the dispatch targets — this is the
// instantiated-type filter, the discriminant that lifts recall without flooding
// precision. The `eval::go_rta` gate asserts no edge targets (Cat).Speak.
type Cat struct{}

func (Cat) Speak() string {
	return "meow"
}

// main is the Go root. It instantiates exactly Dog, converts it to Speaker, and
// performs one dynamic dispatch. RTA resolves main -> (Dog).Speak and nothing else.
func main() {
	var s Speaker = Dog{}
	fmt.Println(s.Speak())
}
