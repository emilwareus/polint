declare const external: (value: unknown) => unknown;

export function tsDomain(flag: boolean, input: string | null | undefined) {
  let label: string;
  let count = 0;
  label = "cold";
  if (input == null) {
    label = "missing";
  } else if (flag) {
    label = "truthy";
  } else {
    label = "falsy";
  }
  while (count < 4) {
    count = count + 1;
  }
  const dynamicTarget: Record<string, unknown> = {};
  dynamicTarget[String(input)] = external(label);
  return { label, count, maybe: input ?? "fallback" };
}
