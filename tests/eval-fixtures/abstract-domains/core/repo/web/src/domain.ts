declare const external: (value: unknown) => unknown;

export function tsDomain(flag: boolean, input: string | null | undefined) {
  // POLINT-FEATURE abstract-domains/ts/maybe-uninitialized-local
  let label: string;
  let count = 0;
  // POLINT-FEATURE abstract-domains/ts/string-constant
  label = "cold";
  // POLINT-FEATURE abstract-domains/ts/nullish-branch
  if (input == null) {
    label = "missing";
  // POLINT-FEATURE abstract-domains/ts/boolean-branch
  } else if (flag) {
    label = "truthy";
  } else {
    label = "falsy";
  }
  while (count < 4) {
    count = count + 1;
  }
  const dynamicTarget: Record<string, unknown> = {};
  // POLINT-FEATURE abstract-domains/ts/dynamic-write-havoc
  dynamicTarget[String(input)] = external(label);
  // POLINT-FEATURE abstract-domains/ts/nullish-coalescing
  return { label, count, maybe: input ?? "fallback" };
}
