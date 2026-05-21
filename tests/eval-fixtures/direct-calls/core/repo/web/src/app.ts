import { importedHelper } from "./helper";

type DynamicBag = {
  [name: string]: (value: string) => string;
};

class Formatter {
  constructor(private readonly prefix: string) {}

  static normalize(value: string): string {
    return value.trim();
  }

  render(value: string): string {
    return `${this.prefix}:${value}`;
  }
}

function localTarget(value: string): string {
  return Formatter.normalize(value);
}

export function handler(input: string, bag: DynamicBag): string {
  const formatter = new Formatter("direct");
  const direct = localTarget(input);
  const imported = importedHelper(direct);
  const member = formatter.render(imported);
  const callable = localTarget;
  const functionValue = callable(member);
  const computed = bag["plugin"](functionValue);
  const viaCall = localTarget.call(null, computed);
  const viaApply = localTarget.apply(null, [viaCall]);
  const viaBind = localTarget.bind(null)(viaApply);
  eval("direct-call fixture");
  import("./lazy");
  return viaBind;
}
