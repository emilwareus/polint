package main

import "fmt"

// handler is a package-level function whose ADDRESS is taken (stored as a func value).
// RTA's address-taken set therefore contains handler, and the indirect call through an
// opaque func() value in main resolves by signature match to handler.
func handler() {
	fmt.Println("handler")
}

// other is a second address-taken func() (same signature). Its address is also taken,
// so the func() dispatch honestly fans out to BOTH handler and other — RTA resolves the
// func-value callsite to the address-taken func() set, not a fabricated single target.
func other() {
	fmt.Println("other")
}

// noise has a DIFFERENT signature (takes an int). Its address is also taken (stored in a
// separate slice), so it exercises the signature filter: the func() indirect call must
// NOT resolve to noise (a func(int)). This keeps func-value resolution honest.
func noise(int) {
	fmt.Println("noise")
}

// main is the Go root. It takes the addresses of handler/other (a []func()) and noise
// (a []func(int)), then invokes a func() element through the slice. SSA cannot resolve
// which element, so the call is an UnresolvedDynamic func-value callsite IN A ROOT; RTA
// resolves it to the address-taken func() set {handler, other}. noise (func(int)) is
// excluded from the func() callsite by its signature.
func main() {
	funcs := []func(){handler, other}
	ints := []func(int){noise}
	funcs[0]()
	ints[0](0)
}
