package twinall

func beginProbe() {}
func cleanupProbe() {}
func Probe(fail bool) {
	beginProbe()
	defer cleanupProbe()
	if fail { return }
}
