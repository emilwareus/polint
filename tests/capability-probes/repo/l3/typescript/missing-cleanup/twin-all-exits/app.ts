function beginProbe(): void {}
function cleanupProbe(): void {}
function logProbe(): void {}
export function probe(fail: boolean): void {
  beginProbe();
  if (fail) { logProbe(); }
  cleanupProbe();
}
