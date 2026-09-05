function seedSinkTs04(value: string): void {}
function seedSanitizeTs04(value: string): string { return "safe"; }
function seedHelperTs04(value: string): void { seedSinkTs04(seedSanitizeTs04(value)); }
export function seedEntryTs04(seedTokenTs04: string): void { seedHelperTs04(seedTokenTs04); }

