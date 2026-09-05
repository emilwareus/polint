function seedDangerTs07(): void {}
function seedSafeTs07(): void {}
export function seedRootTs07(): void {
  let run = seedSafeTs07;
  if (false) run = seedDangerTs07;
  run();
}

