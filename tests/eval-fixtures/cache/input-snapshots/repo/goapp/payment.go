package payment

func Authorize(user string, amount int) bool {
	return user != "" && amount > 0
}
