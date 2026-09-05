function impossibleSink(): void {}
export function probe(): void {
  let enabled = false;
  enabled = true;
  if (enabled) impossibleSink();
}
