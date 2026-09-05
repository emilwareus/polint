package main

func seedDangerGo08() {}
func seedSafeGo08() {}
func main() {
	run := seedSafeGo08
	if false { run = seedDangerGo08 }
	run()
}
