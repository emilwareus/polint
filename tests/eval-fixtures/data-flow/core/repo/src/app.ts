export function sanitize(value: string): string {
  return value.trim();
}

export function identity(value: string): string {
  return value;
}

export function choose(first: string, second: string): string {
  return first;
}

export function overwrite(input: string): string {
  let local = input;
  local = "safe";
  return local;
}

export function conditionalOverwrite(input: string, flag: boolean): string {
  let local = input;
  if (flag) {
    local = "safe";
  }
  return local;
}

export function handler(input: string, fallback: string, flag: boolean): string {
  const local = identity(input);
  choose(local, fallback);
  overwrite(input);
  conditionalOverwrite(input, flag);
  return local;
}
