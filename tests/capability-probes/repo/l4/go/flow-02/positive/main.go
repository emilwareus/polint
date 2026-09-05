package positive

type carrierGo02 struct{ field string }

func seedSinkGo02(value string) {}
func seedEntryGo02(seedTokenGo02 string) {
	carrier := carrierGo02{field: seedTokenGo02}
	seedSinkGo02(carrier.field)
}
