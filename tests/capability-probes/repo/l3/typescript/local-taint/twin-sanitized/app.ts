function dangerousLog(value: string): void {}
function sanitizeProbe(value: string): string { return ""; }
export function probe(probeToken: string): void {
  const staged = probeToken;
  const routed = sanitizeProbe(staged);
  dangerousLog(routed);
}
