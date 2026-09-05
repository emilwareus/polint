function seedDangerTs08(): void {}
function seedSafeTs08(): void {}
class RunnerTs08 {
  seedInvokeTs08(): void { seedSafeTs08(); }
}
export function seedRootTs08(): void {
  const runner = new RunnerTs08();
  let run = () => runner.seedInvokeTs08();
  if (false) run = seedDangerTs08;
  run();
}
