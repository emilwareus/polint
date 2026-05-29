package main

import (
	"fmt"
	"reflect"
)

// reflectCall uses reflection: the Go MIR lowering tags this as an unsupported
// semantic construct (UnresolvedCallReason::Reflection ->
// IdentityCategory::UnsupportedEdge).
func reflectCall(value any) {
	method := reflect.ValueOf(value).MethodByName("Greet")
	method.Call(nil)
}

// callUnknown calls a function with no resolvable target in this unit so the
// callee stays unresolved (MissingSemanticReference ->
// IdentityCategory::UnresolvedEdge).
func callUnknown() {
	missingHelper()
}

func main() {
	reflectCall("loud")
	callUnknown()
	fmt.Println("done")
}
