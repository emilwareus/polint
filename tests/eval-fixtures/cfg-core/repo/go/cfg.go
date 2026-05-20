package cfgcore

func helper(value int) int {
	return value + 1
}

func Route(input int) int {
	total := input
	if input > 0 && helper(input) > 1 {
		total = helper(total)
	}
	for total < 10 {
		total = total + 1
	}
	defer helper(total)
	if total > 50 {
		panic(total)
		return 99
	}
	return total
}
