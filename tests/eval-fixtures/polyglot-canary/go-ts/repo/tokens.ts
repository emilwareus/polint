// The TS half of the polyglot canary. It is deliberately written so the TS frontend
// emits SOLVER-RELEVANT constraints (a `CopyEdge` from a local function alias plus a
// `CallConstraint` for the aliased call), not just zero-constraint LocalFunction calls —
// so "the solver derives NO TS-endpoint edge" is a MEANINGFUL statement about the shared
// solver core (the TS constraints exist and COULD propagate) rather than a vacuous
// "there was nothing to propagate". The TS token-propagation solver (`TsTokensPolicy`) is
// still a STUB in Phase 48, so despite the live TS CopyEdge the unified solver must derive
// NO edge whose endpoints are TS function nodes — proving there is no cross-language
// interference through the shared solver core (D-16). Phase 49 (JS-04) makes the TS driver
// real.

function makeToken(): string {
  return "token";
}

// `aliasMake` aliases makeToken (a CopyEdge in the semantic graph); calling through the
// alias is a CallConstraint that the shared solver core sees — the constraint surface the
// canary asserts the stub does not turn into a derived edge.
const aliasMake = makeToken;

function useToken(token: string): string {
  return token.toUpperCase();
}

function run(): string {
  const value = aliasMake();
  return useToken(value);
}

run();
