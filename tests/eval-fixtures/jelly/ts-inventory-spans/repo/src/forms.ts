function declaredMulti(
  input: string,
): string {
  return input.trim();
}

const expr = function namedExpr(value: string): string {
  return declaredMulti(value);
};

const arrow = (value: string): string => declaredMulti(value);

class Box {
  constructor(value: string) {
    declaredMulti(value);
  }

  method(next: string): string {
    return arrow(next);
  }

  get current(): string {
    return "current";
  }

  set current(next: string) {
    declaredMulti(next);
  }

  static {
    declaredMulti("static");
  }
}

function nestedOuter(flag: boolean): string {
  const nestedArrow = (): string => declaredMulti("nested");
  return flag ? nestedArrow() : arrow("fallback");
}

function tag(strings: TemplateStringsArray, ...values: unknown[]): string {
  return strings.join(String(values.length));
}

export function invoke(
  dynamicSpecifier: string,
  maybe?: (value: string) => string,
): string {
  const widget = new Box("seed");
  tag`hello ${widget.current}`;
  maybe?.("optional");
  import("./static");
  import(dynamicSpecifier);
  require("pkg");
  declaredMulti("direct");
  namedExpr("expr");
  nestedOuter(true);
  return widget.method("done");
}
