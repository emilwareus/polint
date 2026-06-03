function tokenA(): string {
  return "a";
}

function tokenB(): string {
  return "b";
}

function tokenC(): string {
  return "c";
}

function tokenD(): string {
  return "d";
}

function entry(): string {
  return tokenA() + tokenB() + tokenC() + tokenD();
}

entry();
