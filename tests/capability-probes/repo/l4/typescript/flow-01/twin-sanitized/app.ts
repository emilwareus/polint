function seedSinkTs01(value: string): void {}
function seedSanitizeTs01(value: string): string { return "safe"; }
function seedHelperTs01(value: string): void { seedSinkTs01(seedSanitizeTs01(value)); }
export function seedEntryTs01(seedTokenTs01: string): void { seedHelperTs01(seedTokenTs01); }

