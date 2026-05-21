package directcalls

import "reflect"

type worker interface {
	Work(int) int
}

type concreteWorker struct{}

func directFunction(value int) int {
	return value + 1
}

func (concreteWorker) Work(value int) int {
	// POLINT-FEATURE direct-calls/go/direct-function
	return directFunction(value)
}

func reflectInvoke(target any) string {
	// POLINT-FEATURE direct-calls/go/reflection
	return reflect.TypeOf(target).String()
}

func setupMissingPackageShape(missing worker, value int) int {
	// POLINT-FEATURE direct-calls/go/setup-missing-interface
	// setup missing evidence for interface dispatch stays unsupported/setup-sensitive.
	return missing.Work(value)
}

func Process(worker concreteWorker, maybe worker, value int) int {
	// POLINT-FEATURE direct-calls/go/direct-function
	first := directFunction(value)
	// POLINT-FEATURE direct-calls/go/method-call
	second := worker.Work(first)
	fn := directFunction
	// POLINT-FEATURE direct-calls/go/function-value
	third := fn(second) // function value call remains unresolved.
	// POLINT-FEATURE direct-calls/go/goroutine-boundary
	go directFunction(third)
	reflectInvoke(worker)
	return setupMissingPackageShape(maybe, third)
}
