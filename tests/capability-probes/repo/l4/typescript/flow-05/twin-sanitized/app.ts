function seedSinkTs05(value: string): void {}
function seedSanitizeTs05(value: string): string { return "safe"; }
function seedHelperTs05(value: string): void { seedSinkTs05(seedSanitizeTs05(value)); }
export function seedEntryTs05(seedTokenTs05: string): void { seedHelperTs05(seedTokenTs05); }

