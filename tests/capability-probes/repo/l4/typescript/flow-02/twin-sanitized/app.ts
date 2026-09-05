function seedSinkTs02(value: string): void {}
function seedSanitizeTs02(value: string): string { return "safe"; }
export function seedEntryTs02(seedTokenTs02: string): void {
  const carrier = { field: seedSanitizeTs02(seedTokenTs02) };
  seedSinkTs02(carrier.field);
}
