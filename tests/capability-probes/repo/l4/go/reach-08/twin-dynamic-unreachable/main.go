package main

type runnerGo08 struct{}

func seedDangerGo08() {}
func seedSafeGo08() {}
func (r runnerGo08) seedInvokeGo08() { seedSafeGo08() }
func main() {
	run := runnerGo08{}.seedInvokeGo08
	if false { run = seedDangerGo08 }
	run()
}
