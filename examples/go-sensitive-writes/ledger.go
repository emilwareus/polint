package ledger

func Credit(account *Account, amount int) {
	account.Balance += amount
}

func ReplaceBalance(account *Account, next int) {
	account.Balance = next
}

