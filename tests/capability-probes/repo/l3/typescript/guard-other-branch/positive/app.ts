function authorizeProbe(): void {}
function sensitiveWrite(): void {}
export function probe(allowed: boolean): void {
  if (allowed) { authorizeProbe(); return; }
  sensitiveWrite();
}
