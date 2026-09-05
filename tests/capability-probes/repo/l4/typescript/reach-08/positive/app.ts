function seedDangerTs08(): void {}
class RunnerTs08 {
  seedInvokeTs08(): void { seedDangerTs08(); }
}
export function seedRootTs08(): void { new RunnerTs08().seedInvokeTs08(); }
