// Exercises IdentityCategory failure paths from real TS source (Plan 42-03):
// - obj[key]() is a dynamic-property call -> UnresolvedCallReason::DynamicProperty
//   -> IdentityCategory::UnresolvedEdge.
// - eval("...") is an unsupported semantic construct -> UnresolvedCallReason::Eval
//   -> IdentityCategory::UnsupportedEdge.

export function dispatch(obj: Record<string, () => void>, key: string): void {
  obj[key]();
}

export function runEval(code: string): void {
  // eslint-disable-next-line no-eval
  eval(code);
}
