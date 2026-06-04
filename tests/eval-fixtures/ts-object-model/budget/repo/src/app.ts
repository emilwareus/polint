function tokenA(): string {
  return "a";
}

function tokenB(): string {
  return "b";
}

export function budgetEntry(): string {
  const holder = { tokenA, tokenB };
  return holder.tokenA();
}
