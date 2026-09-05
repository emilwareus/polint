package main

type runnerGo08 struct{}

func seedDangerGo08() {}
func (r runnerGo08) seedInvokeGo08() { seedDangerGo08() }
func main() { runnerGo08{}.seedInvokeGo08() }
