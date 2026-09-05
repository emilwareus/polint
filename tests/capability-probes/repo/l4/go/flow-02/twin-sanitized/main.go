package twinsanitized

type carrierGo02 struct{ field string }

func seedSinkGo02(value string) {}
func seedSanitizeGo02(value string) string { return "safe" }
func seedEntryGo02(seedTokenGo02 string) {
	carrier := carrierGo02{field: seedSanitizeGo02(seedTokenGo02)}
	seedSinkGo02(carrier.field)
}
