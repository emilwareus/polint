package twinunrealizable

func seedTargetGo10() {}
func seedCallerGo10() {
	if false { seedTargetGo10() }
}

