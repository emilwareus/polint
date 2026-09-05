function seedSinkTs04(value: string): void {}
function seedInnerTs04(value: string): void { seedSinkTs04(value); }
function seedOuterTs04(value: string): void { seedInnerTs04(value); }
export function seedEntryTs04(seedTokenTs04: string): void {
  if (false) seedOuterTs04(seedTokenTs04);
}
