function seedDangerTs07(): void {}
function seedInnerTs07(): void { seedDangerTs07(); }
function seedOuterTs07(): void { seedInnerTs07(); }
export function seedRootTs07(): void {}
