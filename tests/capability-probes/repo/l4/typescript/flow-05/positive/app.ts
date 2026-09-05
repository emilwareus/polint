function seedSinkTs05(value: string): void {}
function seedCarryTs05(value: string): string { return value; }
export function seedEntryTs05(seedTokenTs05: string): void {
  seedSinkTs05(seedCarryTs05(seedTokenTs05));
}
