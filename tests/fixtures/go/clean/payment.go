package payment

import (
	"errors"
	"strings"
)

type Charge struct {
	ID     string
	Amount int
}

type Ledger struct {
	Denied map[string]bool
}

func Authorize(customer string, charges []Charge) error {
	ledger := Ledger{Denied: map[string]bool{"blocked": true}}
	return ledger.Authorize(customer, charges)
}

func (ledger Ledger) Authorize(customer string, charges []Charge) error {
	if strings.TrimSpace(customer) == "" {
		return ErrInvalid
	}
	if ledger.Denied[customer] {
		return ErrDenied
	}

	total := 0
	for _, charge := range charges {
		switch {
		case charge.Amount < 0:
			return ErrInvalid
		case charge.Amount == 0:
			continue
		default:
			total += charge.Amount
		}
	}
	if total == 0 {
		return errors.New("invalid empty payment")
	}

	return nil
}

var ErrDenied = deniedError{}
var ErrInvalid = invalidError{}

type deniedError struct{}
type invalidError struct{}

func (deniedError) Error() string  { return "denied" }
func (invalidError) Error() string { return "invalid" }
