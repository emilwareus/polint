function seedSinkTs02(value: string): void {}
function seedSanitizeTs02(value: string): string { return "safe"; }
function seedHelperTs02(value: string): void { seedSinkTs02(seedSanitizeTs02(value)); }
export function seedEntryTs02(seedTokenTs02: string): void { seedHelperTs02(seedTokenTs02); }

