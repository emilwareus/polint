package twinunrealizable

func seedSinkGo01(value string) {}
func seedHelperGo01(value string) { seedSinkGo01(value) }
func seedEntryGo01(seedTokenGo01 string) {
	if false { seedHelperGo01(seedTokenGo01) }
}

