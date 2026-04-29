package payment

import (
	"fmt"
	"net/http"
)

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
