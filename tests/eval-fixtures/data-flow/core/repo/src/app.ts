export function sanitize(value: string): string {
  return value.trim();
}

export function handler(input: string): string {
  const local = sanitize(input);
  return local;
}
