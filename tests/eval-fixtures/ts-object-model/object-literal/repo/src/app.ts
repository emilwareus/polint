function propertyTarget(): string {
  return "property";
}

function computedTarget(): string {
  return "computed";
}

function unrelatedExactTarget(): string {
  return "unrelated";
}

function inlineTarget(): string {
  return "inline";
}

function keyName(): string {
  return "dynamic";
}

export function objectEntry(): string {
  const dotHolder = { propertyTarget };
  const fromDot = dotHolder.propertyTarget();

  const stringHolder = { "propertyTarget": propertyTarget };
  const fromString = stringHolder["propertyTarget"]();

  const dynamic = keyName();
  const computedHolder = {
    [dynamic]: computedTarget,
    unrelatedExactTarget,
  };
  const fromComputed = computedHolder[dynamic]();

  const fromInline = ({ inlineTarget }).inlineTarget();

  const methodHolder = {
    methodTarget(): string {
      return "method";
    },
  };
  const fromMethod = methodHolder.methodTarget();

  return fromDot + fromString + fromComputed + fromInline + fromMethod;
}

objectEntry();
