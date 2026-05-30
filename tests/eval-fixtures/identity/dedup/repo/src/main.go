package main

import "fmt"

// helper is called from two callsites below. This Go repo has no true semantic
// duplicates, so the live fixture asserts the deterministic multiplicity = 1
// (D-09, D-10) and proves dedup is order-independent and byte-stable across run,
// file, and provider order (D-11). The multiplicity = 2 collapse contract for
// genuine semantic duplicates is proven by the co-located dedup unit tests
// (analysis::identity::dedup), not by this fixture.
func helper() {
	fmt.Println("work")
}

func first() {
	helper()
}

func second() {
	helper()
}

func main() {
	first()
	second()
}
