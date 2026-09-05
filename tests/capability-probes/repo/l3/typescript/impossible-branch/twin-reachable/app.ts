function impossibleSink(): void {}
export function probe(): void {
  const enabled = true;
  if (enabled) impossibleSink();
}
