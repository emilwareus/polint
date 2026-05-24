export function readName(input: { name?: string }, fallback: string): string {
  if (input.name != null) {
    const narrowed = input.name;
    return narrowed;
  }
  const box = { value: input.name ?? fallback };
  const alias = box;
  return alias.value;
}
