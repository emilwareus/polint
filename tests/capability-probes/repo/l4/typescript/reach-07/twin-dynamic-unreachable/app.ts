function seedDangerTs07(): void {}
function seedSafeTs07(): void {}
export function seedRootTs07(): void {
  const table: Record<string, () => void> = { safe: seedSafeTs07 };
  if (false) table.danger = seedDangerTs07;
  table.safe();
}
