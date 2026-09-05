function seedSinkTs01(value: string): void {}
function seedHelperTs01(value: string): void { seedSinkTs01(value); }
export function seedEntryTs01(seedTokenTs01: string): void { seedHelperTs01(seedTokenTs01); }

