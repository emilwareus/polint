package twinunrealizable

func seedSinkGo05(value string) {}
func seedHelperGo05(value string) { seedSinkGo05(value) }
func seedEntryGo05(seedTokenGo05 string) {
	if false { seedHelperGo05(seedTokenGo05) }
}

