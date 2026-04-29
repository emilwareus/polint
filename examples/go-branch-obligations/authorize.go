package payments

func Authorize(amount int, charge func(int) error, audit func(int) error) error {
	if amount <= 0 {
		return ErrInvalidAmount
	}
	if err := audit(amount); err != nil {
		return err
	}
	if err := charge(amount); err != nil {
		return err
	}
	return nil
}

var ErrInvalidAmount = errInvalidAmount{}

type errInvalidAmount struct{}

func (errInvalidAmount) Error() string { return "invalid amount" }
