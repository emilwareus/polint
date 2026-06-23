// `formatUser` is an exported symbol. It is imported by `src/app.ts`, so when
// `src/api.ts` changes, the complex `review/public-api-change` rule flags it as
// a public-API surface to review.
export function formatUser(name: string): string {
  return name.trim();
}
