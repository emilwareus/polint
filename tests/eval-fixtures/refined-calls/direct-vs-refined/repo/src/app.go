package refinedcalls

type worker interface {
	Work(int) int
}

func Process(worker worker, value int) int {
	return helper(worker.Work(value))
}

func helper(value int) int {
	return value + 1
}
