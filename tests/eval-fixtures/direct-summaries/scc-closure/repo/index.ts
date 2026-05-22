export function leafTs(): number {
  return 1;
}

export function callerTs(): number {
  return leafTs();
}

export function crossLanguageLike(value: number): unknown {
  return (globalThis as any).goProvided(value);
}
