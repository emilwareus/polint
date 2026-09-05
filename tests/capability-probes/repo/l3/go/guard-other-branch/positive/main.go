package positive

func authorizeProbe() {}
func sensitiveWrite() {}
func Probe(allowed bool) {
	if allowed { authorizeProbe(); return }
	sensitiveWrite()
}
