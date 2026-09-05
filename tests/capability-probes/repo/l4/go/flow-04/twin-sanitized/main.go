package twinsanitized

func seedSinkGo04(value string) {}
func seedSanitizeGo04(value string) string { return "safe" }
func seedHelperGo04(value string) { seedSinkGo04(seedSanitizeGo04(value)) }
func seedEntryGo04(seedTokenGo04 string) { seedHelperGo04(seedTokenGo04) }

