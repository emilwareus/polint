package positive

func dangerousLog(value string) {}
func sanitizeProbe(value string) string { return "" }
func Probe(probeToken string) {
	staged := probeToken
	routed := staged
	dangerousLog(routed)
}
