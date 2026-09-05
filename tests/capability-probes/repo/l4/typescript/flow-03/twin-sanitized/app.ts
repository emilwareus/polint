function seedSinkTs03(value: string): void {}
function seedSanitizeTs03(value: string): string { return "safe"; }
function seedHelperTs03(value: string): void { seedSinkTs03(seedSanitizeTs03(value)); }
export function seedEntryTs03(seedTokenTs03: string): void { seedHelperTs03(seedTokenTs03); }

