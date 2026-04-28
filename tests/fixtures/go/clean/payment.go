package payment

func Authorize(ok bool) error {
	if !ok {
		return ErrDenied
	}
	return nil
}

var ErrDenied = deniedError{}

type deniedError struct{}

func (deniedError) Error() string { return "denied" }
