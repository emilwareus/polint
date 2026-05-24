export function readName(input: { name?: string }, fallback: string): string {
  const box = { value: input.name ?? fallback };
  const alias = box;
  return alias.value;
}
