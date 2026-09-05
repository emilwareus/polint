function beginProbe(): void {}
function cleanupProbe(): void {}
export function probe(fail: boolean): void {
  beginProbe();
  cleanupProbe();
  if (fail) return;
}
