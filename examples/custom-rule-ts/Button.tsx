// This example repo has one local polint rule: local/no-product-hex-colors.
// The rule catches product UI colors that are written as raw hex literals
// instead of design tokens. The component below intentionally does that once.
export function Button() {
  // Intentional violation: product UI code should use a token, not a hex value.
  const danger = "#ff00aa";
  return <button style={{ color: danger }}>Delete</button>;
}
