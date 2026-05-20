package semanticmir

var globalSink int

type User struct {
	Tokens []int
}

func helper(value int) int {
	return value + 1
}

func Process(user User, index int) int {
	local := user.Tokens[index]
	globalSink = helper(local)
	if local > 0 {
		local = helper(local)
	}
	defer helper(local)
	return local
}
