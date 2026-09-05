package main

func seedDangerGo07() {}
func seedSafeGo07() {}
func main() {
	run := seedSafeGo07
	if false { run = seedDangerGo07 }
	run()
}
