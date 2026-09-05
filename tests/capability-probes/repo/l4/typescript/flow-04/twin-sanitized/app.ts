function seedSinkTs04(value: string): void {}
function seedSanitizeTs04(value: string): string { return "safe"; }
function seedInnerTs04(value: string): void { seedSinkTs04(seedSanitizeTs04(value)); }
function seedOuterTs04(value: string): void { seedInnerTs04(value); }
export function seedEntryTs04(seedTokenTs04: string): void { seedOuterTs04(seedTokenTs04); }
