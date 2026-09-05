type Item = { value: string };
function useNull(value: string): void {}
export function probe(input: Item | null): void {
  if (input !== null) return;
  useNull(input.value);
}
