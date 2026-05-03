// This example repo has one local polint rule: local/go-branch-obligations.
// The rule finds important Go branches, then heuristically checks whether a
// nearby test appears to cover them. This file intentionally has uncovered
// error branches.
package payments

func Authorize(amount int, charge func(int) error, audit func(int) error) error {
	if amount <= 0 {
		// Intentional violation: no nearby test evidence exercises this branch.
		return ErrInvalidAmount
	}
	if err := audit(amount); err != nil {
		// Intentional violation: this error branch also lacks matching evidence.
		return err
	}
	if err := charge(amount); err != nil {
		// Intentional violation: the local rule should ask for a test case here.
		return err
	}
	return nil
}

var ErrInvalidAmount = errInvalidAmount{}

type errInvalidAmount struct{}

func (errInvalidAmount) Error() string { return "invalid amount" }
