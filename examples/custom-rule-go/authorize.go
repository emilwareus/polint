// This example repo has one local polint rule:
// local/require-error-branch-tests. The rule looks for Go error branches that
// do not have nearby test evidence. This file intentionally has an error branch
// and no companion test file.
package payments

func Authorize(err error) error {
	if err != nil {
		// Intentional violation: this error branch needs local test evidence.
		return err
	}
	return nil
}
