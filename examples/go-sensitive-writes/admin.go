package ledger

func ResetBalanceFromAdmin(account *Account) {
	account.Balance = 0
}
