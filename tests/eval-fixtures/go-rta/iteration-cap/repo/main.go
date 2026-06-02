package main

import "fmt"

// Shape is the dispatched interface. THREE concrete types implement Area(), and all
// three are instantiated/converted-to-interface in the reachable program, so a SINGLE
// interface invoke `s.Area()` has three candidate callees under RTA.
type Shape interface {
	Area() int
}

// Circle, Square, and Triangle are all instantiated below and all implement Area(),
// so the one dynamic dispatch fans out to three candidate callees. With a tight
// `[solver.go] max_candidates_per_callsite = 1`, resolving this callsite exceeds the
// per-callsite candidate cap and latches BudgetStatus::BudgetExceeded — the GO-05
// criterion-2 proof that runaway dispatch fan-out is signalled, not silently dropped.
type Circle struct{}

func (Circle) Area() int { return 1 }

type Square struct{}

func (Square) Area() int { return 2 }

type Triangle struct{}

func (Triangle) Area() int { return 3 }

// shapes returns a slice holding all three instantiated shapes (each converted to the
// Shape interface — the RTA rapid-type set gains Circle, Square, and Triangle).
func shapes() []Shape {
	return []Shape{Circle{}, Square{}, Triangle{}}
}

// main is the Go root. It performs one dynamic dispatch on a Shape whose dynamic type
// is any of the three instantiated implementers, so RTA's candidate set for the single
// `s.Area()` callsite is {Circle.Area, Square.Area, Triangle.Area} — three candidates
// for one callsite, which a `max_candidates_per_callsite = 1` cap cannot admit.
func main() {
	total := 0
	for _, s := range shapes() {
		total += s.Area()
	}
	fmt.Println(total)
}
