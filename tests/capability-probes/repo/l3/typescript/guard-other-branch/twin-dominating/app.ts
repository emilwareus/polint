function authorizeProbe(): void {}
function sensitiveWrite(): void {}
export function probe(allowed: boolean): void {
  authorizeProbe();
  if (allowed) return;
  sensitiveWrite();
}
