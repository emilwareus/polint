package twinreachable

func impossibleSink() {}
func Probe() {
	enabled := false
	enabled = true
	if enabled { impossibleSink() }
}
