package main

import "fmt"

// helper is called from two semantically-identical callsites below so that the
// identity provider's dedup layer collapses them into one record with
// multiplicity = 2 (D-09, D-10). The fixture proves dedup is order-independent
// and byte-stable across run, file, and provider order (D-11).
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
