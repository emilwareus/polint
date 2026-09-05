package twinsanitized

func seedSinkGo01(value string) {}
func seedSanitizeGo01(value string) string { return "safe" }
func seedHelperGo01(value string) { seedSinkGo01(seedSanitizeGo01(value)) }
func seedEntryGo01(seedTokenGo01 string) { seedHelperGo01(seedTokenGo01) }

