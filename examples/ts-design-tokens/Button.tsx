// This example repo has one local polint rule: local/no-raw-colors.
// The rule catches raw color literals in TSX strings and JSX attributes so UI
// code moves colors into design tokens. The component below intentionally has
// two raw colors.
export function Button() {
  // Intentional violation: use a design token instead of this raw hex literal.
  const accent = "#ff00aa";

  return (
    // Intentional violation: this JSX attribute also embeds a raw color.
    <button style={{ color: accent }} data-color="#00ff00">
      Pay
    </button>
  );
}
