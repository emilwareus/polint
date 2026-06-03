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

// noise has a DIFFERENT signature (takes an int). Its address IS taken (stored in a
// separate []func(int) slice, so it is genuinely in RTA's address-taken candidate set),
// but that slice is NEVER invoked — there is NO func(int) callsite anywhere reachable. So
// noise exercises the SIGNATURE FILTER from the candidate side: it is an address-taken
// func value, yet the only func-value callsite (`funcs[0]()`, a func()) must NOT resolve
// to it because its signature does not match. The `eval::go_rta` gate asserts no derived
// edge targets noise, proving the filter excludes a same-set-but-wrong-signature candidate
// rather than flooding it in.
func noise(int) {
	fmt.Println("noise")
}

// main is the Go root. It takes the addresses of handler/other (a []func()) and noise
// (a []func(int)). It invokes a func() element through the func() slice — an
// UnresolvedDynamic func-value callsite IN A ROOT that RTA resolves to the address-taken
// func() set {handler, other}. The []func(int){noise} slice is address-taken but NOT
// invoked (no func(int) callsite), so noise stays in the candidate set yet is never a
// resolved target — the signature filter excludes it from the func() callsite.
func main() {
	funcs := []func(){handler, other}
	ints := []func(int){noise}
	_ = ints
	funcs[0]()
}
