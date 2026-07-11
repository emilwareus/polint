// Determinism-gate TS fixture (, D-24).
//
// `entry` is exported, so it is a reachability root (RootKind::Exported). It
// directly calls usedHelper (a reachable call site). orphanFn is neither
// exported nor called from any root, so its call site orphanFn -> orphanHelper
// is OUTSIDE the reachable graph (in_reachable_graph = false) — the unreachable
// mark the gate asserts.

function usedHelper(value: string): string {
  return value.toUpperCase();
}

function orphanHelper(value: string): string {
  return value.toLowerCase();
}

// orphanFn is never reached from a root; its outgoing call is unreachable.
function orphanFn(value: string): string {
  return orphanHelper(value);
}

// entry is the exported TS root. It directly calls usedHelper (reachable).
export function entry(input: string): string {
  return usedHelper(input);
}
