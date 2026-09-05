function impossibleSink(): void {}
export function probe(): void {
  const enabled = false;
  if (enabled) impossibleSink();
}
