package twindominating

func authorizeProbe() {}
func sensitiveWrite() {}
func Probe(allowed bool) {
	authorizeProbe()
	if allowed { return }
	sensitiveWrite()
}
