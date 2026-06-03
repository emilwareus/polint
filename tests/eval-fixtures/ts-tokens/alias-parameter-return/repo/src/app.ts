function aliasTarget(): string {
  return "alias";
}

function assignedTarget(): string {
  return "assigned";
}

function parameterTarget(): string {
  return "parameter";
}

function returnTarget(): string {
  return "return";
}

function closureTarget(): string {
  return "closure";
}

function propertyTarget(): string {
  return "property";
}

const aliasCall = aliasTarget;
const assignedCall = assignedTarget;

function invokeCallback(cb: () => string): string {
  return cb();
}

function makeReturn(): () => string {
  return returnTarget;
}

function makeClosure(): () => string {
  const captured = closureTarget;
  return function closureReturned(): string {
    return captured();
  };
}

export function entry(): string {
  const first = aliasCall();
  const second = assignedCall();
  const parameter = invokeCallback(parameterTarget);
  const returned = makeReturn();
  const third = returned();
  const closure = makeClosure();
  const fourth = closure();
  const holder = { propertyTarget };
  const fifth = holder["propertyTarget"]();

  return first + second + parameter + third + fourth + fifth;
}

entry();
