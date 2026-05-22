package sccclosure

func leaf() int {
	return 1
}

func middle() int {
	return leaf()
}

func top() int {
	return middle()
}

func pingGo(n int) int {
	if n <= 0 {
		return 0
	}
	return pongGo(n - 1)
}

func pongGo(n int) int {
	if n <= 0 {
		return 1
	}
	return pingGo(n - 1)
}
