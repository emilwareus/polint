package abstractdomains

func unknownInt() int {
	return 7
}

func unsupported(any interface{}) interface{} {
	return any
}

func GoDomain(flag bool, input *int) string {
	var out string
	var count int
	out = "cold"
	if input == nil {
		out = "nil"
	} else if flag {
		out = "warm"
		count = 1
	} else {
		out = "cool"
	}
	for count < unknownInt() {
		count = count + 1
		if count > 2 {
			break
		}
	}
	_ = unsupported(input)
	return out
}
