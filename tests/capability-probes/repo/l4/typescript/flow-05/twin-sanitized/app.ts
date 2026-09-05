function seedSinkTs05(value: string): void {}
function seedSanitizeTs05(value: string): string { return "safe"; }
function seedCarryTs05(value: string): string { return value; }
export function seedEntryTs05(seedTokenTs05: string): void {
  seedSinkTs05(seedSanitizeTs05(seedCarryTs05(seedTokenTs05)));
}
