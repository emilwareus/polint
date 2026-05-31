// Semantic-graph TS/JS snapshot fixture (Phase 44 Plan 03, GRAPH-02, D-12).
//
// `entry` calls `greet` (a direct call site -> Call edge + CallConstraint) and
// copies the result into locals (value copies -> CopyEdge constraints), exercising
// the node/edge/constraint kinds the minimal projection emits today.

function greet(name: string): string {
  return "hello " + name;
}

export function entry(input: string): string {
  const message = greet(input);
  const other = message;
  return other;
}
