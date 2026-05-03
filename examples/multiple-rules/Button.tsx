// This example repo has two local polint rules registered by one Cargo crate.
// local/no-raw-colors catches this raw TSX color literal.
export function Button() {
  // Intentional violation: use a design token instead of this raw hex value.
  return <button data-color="#ff00aa">Pay</button>;
}
