class HolderTs10 {
  seedTargetTs10(): void {}
}
export function seedCallerTs10(): void {
  if (false) new HolderTs10().seedTargetTs10();
}
