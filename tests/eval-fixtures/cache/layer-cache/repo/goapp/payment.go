package payment

import "strings"

func Authorize(user string, amount int) bool {
	return strings.TrimSpace(user) != "" && amount > 0
}
