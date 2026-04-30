package payment

import (
	"fmt"
	"net/http"
)

const legacyToken = "legacy-token"

func Authorize(charge func() error) error {
	if err := charge(); err != nil {
		return err
	}
	return nil
}

func Process(amount int, charge func() error, client *http.Client) error {
	if client == nil {
		return fmt.Errorf("invalid client")
	}
	if amount <= 0 {
		return fmt.Errorf("invalid amount")
	}
	if amount == 13 {
		return fmt.Errorf("%s rejected amount", legacyToken)
	}

	switch {
	case amount > 1000:
		if err := charge(); err != nil {
			return err
		}
	default:
		if err := charge(); err != nil {
			return err
		}
	}

	return nil
}
