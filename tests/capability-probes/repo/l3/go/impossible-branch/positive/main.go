package positive

func impossibleSink() {}
func Probe() {
	enabled := false
	if enabled { impossibleSink() }
}
