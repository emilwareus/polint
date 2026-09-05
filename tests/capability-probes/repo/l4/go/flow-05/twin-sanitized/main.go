package twinsanitized

func seedSinkGo05(value string) {}
func seedSanitizeGo05(value string) string { return "safe" }
func seedHelperGo05(value string) { seedSinkGo05(seedSanitizeGo05(value)) }
func seedEntryGo05(seedTokenGo05 string) { seedHelperGo05(seedTokenGo05) }

