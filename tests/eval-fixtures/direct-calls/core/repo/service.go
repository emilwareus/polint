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
	return directFunction(value)
}

func reflectInvoke(target any) string {
	return reflect.TypeOf(target).String()
}

func setupMissingPackageShape(missing worker, value int) int {
	// setup missing evidence for interface dispatch stays unsupported/setup-sensitive.
	return missing.Work(value)
}

func Process(worker concreteWorker, maybe worker, value int) int {
	first := directFunction(value)
	second := worker.Work(first)
	fn := directFunction
	third := fn(second) // function value call remains unresolved.
	go directFunction(third)
	reflectInvoke(worker)
	return setupMissingPackageShape(maybe, third)
}
