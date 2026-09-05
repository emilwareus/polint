package twinunrealizable

func seedSinkGo04(value string) {}
func seedInnerGo04(value string) { seedSinkGo04(value) }
func seedOuterGo04(value string) { seedInnerGo04(value) }
func seedEntryGo04(seedTokenGo04 string) {
	if false { seedOuterGo04(seedTokenGo04) }
}
