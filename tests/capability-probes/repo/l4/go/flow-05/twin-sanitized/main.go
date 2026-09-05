package twinsanitized

func seedSinkGo05(value string) {}
func seedSanitizeGo05(value string) string { return "safe" }
func seedCarryGo05(value string) string { return value }
func seedEntryGo05(seedTokenGo05 string) {
	seedSinkGo05(seedSanitizeGo05(seedCarryGo05(seedTokenGo05)))
}
