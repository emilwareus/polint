// The TS half of the polyglot canary. It declares local functions and a direct
// callsite so the TS frontend genuinely analyzes TS functions in the same run as the
// Go RTA driver. The TS token-propagation solver (`TsTokensPolicy`) is still a STUB
// in Phase 48, so the unified solver must derive NO TS-endpoint edge — the canary
// asserts this to prove there is no cross-language interference through the shared
// solver core (D-16). Phase 49 (JS-04) makes the TS driver real.

function makeToken(): string {
  return "token";
}

function useToken(token: string): string {
  return token.toUpperCase();
}

const value = makeToken();
useToken(value);
