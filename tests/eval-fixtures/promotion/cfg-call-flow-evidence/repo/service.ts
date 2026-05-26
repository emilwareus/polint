export function handler(input: string): string {
  if (input.length > 0) {
    return sink(sanitize(input));
  }

  return "empty";
}

function sanitize(value: string): string {
  return value.trim();
}

function sink(value: string): string {
  return value;
}
