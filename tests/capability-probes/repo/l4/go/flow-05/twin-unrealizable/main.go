package twinunrealizable

func seedSinkGo05(value string) {}
func seedCarryGo05(value string) string { return value }
func seedEntryGo05(seedTokenGo05 string) {
	if false { seedSinkGo05(seedCarryGo05(seedTokenGo05)) }
}
