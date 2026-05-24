export function choose(left: { id: string }, right: { id: string }): string {
  const selected = left.id.length > 0 ? left : right;
  return selected.id;
}
