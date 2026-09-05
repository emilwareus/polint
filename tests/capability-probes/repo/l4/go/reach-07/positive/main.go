package main

func seedDangerGo07() {}
func seedInnerGo07() { seedDangerGo07() }
func seedOuterGo07() { seedInnerGo07() }
func main() { seedOuterGo07() }
