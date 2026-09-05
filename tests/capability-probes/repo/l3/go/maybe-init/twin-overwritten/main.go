package twinoverwritten

func useMaybeInit(value string) {}
func Probe(flag bool) {
	value := "unsafe"
	value = "safe"
	if flag { value = "also-safe" }
	useMaybeInit(value)
}
