package twinsanitized

func seedSinkGo02(value string) {}
func seedSanitizeGo02(value string) string { return "safe" }
func seedHelperGo02(value string) { seedSinkGo02(seedSanitizeGo02(value)) }
func seedEntryGo02(seedTokenGo02 string) { seedHelperGo02(seedTokenGo02) }

