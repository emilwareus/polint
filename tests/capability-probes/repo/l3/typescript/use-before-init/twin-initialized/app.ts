function useBeforeInit(value: string): void {}
export function probe(flag: boolean): void {
  let value = "ready";
  if (flag) value = "updated";
  useBeforeInit(value);
}
