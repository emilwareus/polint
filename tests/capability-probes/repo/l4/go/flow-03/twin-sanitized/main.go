package twinsanitized

func seedSinkGo03(value string) {}
func seedSanitizeGo03(value string) string { return "safe" }
func seedHelperGo03(value string) { seedSinkGo03(seedSanitizeGo03(value)) }
func seedEntryGo03(seedTokenGo03 string) { seedHelperGo03(seedTokenGo03) }

