package positive

func dangerousLog(value string) {}
func sanitizeProbe(value string) string { return "" }
func Probe(probeToken string) { dangerousLog(probeToken) }
