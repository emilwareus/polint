function seedSinkTs03(value: string): void {}
function seedSanitizeTs03(value: string): string { return "safe"; }
export function seedEntryTs03(seedTokenTs03: string): void {
  const values = [seedSanitizeTs03(seedTokenTs03)];
  seedSinkTs03(values[0]);
}
