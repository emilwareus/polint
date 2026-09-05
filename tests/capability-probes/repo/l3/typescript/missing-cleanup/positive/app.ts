function beginProbe(): void {}
function cleanupProbe(): void {}
export function probe(fail: boolean): void {
  beginProbe();
  if (fail) return;
  cleanupProbe();
}
