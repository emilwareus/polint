package twinsanitized

func seedSinkGo04(value string) {}
func seedSanitizeGo04(value string) string { return "safe" }
func seedInnerGo04(value string) { seedSinkGo04(seedSanitizeGo04(value)) }
func seedOuterGo04(value string) { seedInnerGo04(value) }
func seedEntryGo04(seedTokenGo04 string) { seedOuterGo04(seedTokenGo04) }
