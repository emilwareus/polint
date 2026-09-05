package main

func seedDangerGo07() {}
func seedSafeGo07() {}
func main() {
	table := map[string]func(){"safe": seedSafeGo07}
	if false { table["danger"] = seedDangerGo07 }
	table["safe"]()
}
