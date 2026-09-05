package positive

func seedSinkGo05(value string) {}
func seedCarryGo05(value string) string { return value }
func seedEntryGo05(seedTokenGo05 string) { seedSinkGo05(seedCarryGo05(seedTokenGo05)) }
