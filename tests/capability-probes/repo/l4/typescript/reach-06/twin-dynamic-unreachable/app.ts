function seedDangerTs06(): void {}
function seedSafeTs06(): void {}
export function seedRootTs06(): void {
  let run = seedSafeTs06;
  if (false) run = seedDangerTs06;
  run();
}

