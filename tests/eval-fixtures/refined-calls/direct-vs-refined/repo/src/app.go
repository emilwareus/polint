package refinedcalls

type worker interface {
	Work(int) int
}

func Process(worker worker, value int) int {
	return worker.Work(value)
}
