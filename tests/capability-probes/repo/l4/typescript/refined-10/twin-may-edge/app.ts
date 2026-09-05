function seedTargetTs10(): void {}
export function seedCallerTs10(): void {
  const chosen = seedTargetTs10;
  chosen();
}

