package twinunrealizable

func seedTargetGo09() {}
func seedCallerGo09() {
	if false { seedTargetGo09() }
}

