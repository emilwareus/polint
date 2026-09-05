package twinreachable

func impossibleSink() {}
func Probe() {
	enabled := true
	if enabled { impossibleSink() }
}
