// This example repo has one local polint rule: local/ts-cyclomatic-complexity.
// The rule reports TS/JS functions whose branch count exceeds the configured
// max. This function intentionally has two if statements while .polint.toml
// sets max = 1.
export function label(status: string, admin: boolean) {
  if (admin) {
    // This branch contributes to the function's cyclomatic complexity.
    return "admin";
  }
  if (status === "paid") {
    // This second branch pushes the function over the example threshold.
    return "paid";
  }
  return "draft";
}
