function seedDangerTs08(): void {}
function seedSafeTs08(): void {}
export function seedRootTs08(): void {
  let run = seedSafeTs08;
  if (false) run = seedDangerTs08;
  run();
}

