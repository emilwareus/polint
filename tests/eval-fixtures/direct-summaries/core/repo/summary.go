package summarytest

var globalCounter int

// State is a receiver for MutatesReceiver.
type State struct {
	field int
}

// POLINT-FEATURE direct-summaries/go/panic-does-not-return
func AlwaysPanics() {
	panic("fail")
}

// POLINT-FEATURE direct-summaries/go/param-to-return-tito
func ReturnsParam(x int) int {
	return x
}

// POLINT-FEATURE direct-summaries/go/receiver-mutation
func (s *State) MutatesReceiver() {
	s.field = 42
}

// POLINT-FEATURE direct-summaries/go/global-read
func ReadsGlobal() int {
	return globalCounter
}

// POLINT-FEATURE direct-summaries/go/unresolved-call
func CallsUnresolved(fn func()) {
	fn()
}

// POLINT-FEATURE direct-summaries/go/pure-function
func PureFunction(a, b int) int {
	return a + b
}
