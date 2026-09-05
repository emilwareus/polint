package positive

func beginProbe() {}
func cleanupProbe() {}
func Probe(fail bool) {
	beginProbe()
	if fail { return }
	cleanupProbe()
}
