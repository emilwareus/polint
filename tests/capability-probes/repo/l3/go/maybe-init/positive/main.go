package positive

func useMaybeInit(value string) {}
func Probe(flag bool) {
	value := "unsafe"
	if flag { value = "safe" }
	useMaybeInit(value)
}
