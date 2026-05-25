function localTarget(value: string): string {
  return value.trim();
}

export function handler(input: string): string {
  return localTarget(input);
}
