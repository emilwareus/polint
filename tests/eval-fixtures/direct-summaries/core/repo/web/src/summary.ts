// POLINT-FEATURE direct-summaries/ts/throw-does-not-return
export function throwsError(): never {
  throw new Error("fail");
}

// POLINT-FEATURE direct-summaries/ts/param-to-return-tito
export function identity(x: number): number {
  return x;
}

// POLINT-FEATURE direct-summaries/ts/param-write
export function writesParam(arr: number[], val: number): void {
  arr.push(val);
}

// POLINT-FEATURE direct-summaries/ts/dynamic-callback
export function callsDynamic(cb: () => void): void {
  cb();
}

// POLINT-FEATURE direct-summaries/ts/no-effects
export function noEffects(): string {
  return "hello";
}
