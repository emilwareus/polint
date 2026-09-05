class HolderTs10 {
  seedTargetTs10(): void {}
}
export function seedCallerTs10(): void {
  const holder = new HolderTs10();
  const chosen = () => holder.seedTargetTs10();
  chosen();
}
