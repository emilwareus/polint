package payment

func Authorize(charge func() error) error {
	if err := charge(); err != nil {
		return err
	}
	return nil
}
