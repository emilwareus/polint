package twinsanitized

func seedSinkGo03(value string) {}
func seedSanitizeGo03(value string) string { return "safe" }
func seedEntryGo03(seedTokenGo03 string) {
	values := []string{seedSanitizeGo03(seedTokenGo03)}
	seedSinkGo03(values[0])
}
