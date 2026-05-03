// This example repo has one local polint rule: local/no-raw-colors.
// The rule flags hard-coded color literals in TSX so UI code uses design
// tokens instead. The component below intentionally contains one raw hex color.
export function Button() {
  // Intentional violation: this data attribute embeds a raw color literal.
  return <button data-color="#ff00aa">Pay</button>;
}
