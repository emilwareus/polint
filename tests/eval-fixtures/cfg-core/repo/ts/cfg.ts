function helper(value: number): number {
  return value + 1;
}

export function route(input: number, options?: { enabled?: boolean }): number {
  let total = input;
  if (options?.enabled && (input ?? 0) > 0) {
    total = helper(total);
  }
  for (; total < 10; total++) {
    total = helper(total);
  }
  try {
    if (total > 50) {
      throw new Error("too high");
      return -1;
    }
  } finally {
    total = helper(total);
  }
  return total;
}
