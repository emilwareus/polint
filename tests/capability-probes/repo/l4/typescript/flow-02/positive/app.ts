function seedSinkTs02(value: string): void {}
export function seedEntryTs02(seedTokenTs02: string): void {
  const carrier = { field: seedTokenTs02 };
  seedSinkTs02(carrier.field);
}
