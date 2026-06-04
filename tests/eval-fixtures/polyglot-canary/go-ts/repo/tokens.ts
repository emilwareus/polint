// The TS half of the polyglot canary. It is deliberately written so the TS frontend
// emits solver-relevant constraints: a CopyEdge from a local function alias plus a
// CallConstraint for the aliased call. Phase 49's TsTokensPolicy should resolve that
// into an intra-TS token call edge, while the mixed Go+TS solver output must still
// contain no Go<->TS edge.

function makeToken(): string {
  return "token";
}

// `aliasMake` aliases makeToken (a CopyEdge in the semantic graph); calling through the
// alias is a CallConstraint that the shared solver core sees.
const aliasMake = makeToken;

function useToken(token: string): string {
  return token.toUpperCase();
}

function run(): string {
  const value = aliasMake();
  return useToken(value);
}

run();
