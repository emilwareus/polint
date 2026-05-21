package abstractdomains

func unknownInt() int {
	return 7
}

func unsupported(any interface{}) interface{} {
	return any
}

func GoDomain(flag bool, input *int) string {
	// POLINT-FEATURE abstract-domains/go/initialized-locals
	var out string
	var count int
	// POLINT-FEATURE abstract-domains/go/string-constant
	out = "cold"
	// POLINT-FEATURE abstract-domains/go/nil-branch
	if input == nil {
		out = "nil"
	// POLINT-FEATURE abstract-domains/go/boolean-branch
	} else if flag {
		out = "warm"
		count = 1
	} else {
		out = "cool"
	}
	// POLINT-FEATURE abstract-domains/go/loop-widening
	for count < unknownInt() {
		count = count + 1
		if count > 2 {
			break
		}
	}
	// POLINT-FEATURE abstract-domains/go/unknown-call-havoc
	_ = unsupported(input)
	return out
}
