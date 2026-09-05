package twininitialized

func useMaybeInit(value string) {}
func Probe(flag bool) {
	value := "safe"
	if flag { value = "also-safe" }
	useMaybeInit(value)
}
