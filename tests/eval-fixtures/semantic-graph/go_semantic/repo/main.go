package main

type runner interface {
	Run()
}

type worker struct{}

func init() {}

func (worker) Run() {}

func direct() {}

func callDirect() {
	direct()
}

func callInterface(r runner) {
	r.Run()
}

func main() {
	callDirect()
	callInterface(worker{})
}
