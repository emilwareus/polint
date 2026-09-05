function useBeforeInit(value: string): void {}
export function probe(flag: boolean): void {
  let value: string;
  if (flag) { value = "ready"; } else { value = "fallback"; }
  useBeforeInit(value);
}
