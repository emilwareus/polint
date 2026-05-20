type User = {
  tokens: string[];
};

declare const window: {
  store: Record<string, string>;
};

function format(value: string): string {
  return value.trim();
}

export function render(user: User, index: number): string {
  const token = user.tokens[index];
  let local = token;
  if (local) {
    local = format(local);
  }
  const returned = format(local);
  window.store[index] = returned;
  eval("semantic-mir");
  return returned;
}
