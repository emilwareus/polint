package twinallexits

func beginProbe() {}
func cleanupProbe() {}
func logProbe() {}
func Probe(fail bool) {
	beginProbe()
	if fail { logProbe() }
	cleanupProbe()
}
