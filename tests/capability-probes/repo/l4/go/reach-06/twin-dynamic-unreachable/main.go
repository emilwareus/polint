package main

func seedDangerGo06() {}
func seedSafeGo06() {}
func main() {
	run := seedSafeGo06
	if false { run = seedDangerGo06 }
	run()
}
