class BaseModel {
  baseMethod(): string {
    return "base";
  }
}

class ChildModel extends BaseModel {
  childMethod(): string {
    return "child";
  }

  get accessorMethod(): () => string {
    return accessorTarget;
  }

  set accessorMethod(value: () => string) {
    value();
  }
}

function accessorTarget(): string {
  return "accessor";
}

const lexicalArrow = () => this;

function boundTarget(): string {
  return "bound";
}

export function prototypeEntry(): string {
  const child = new ChildModel();
  const fromChild = child.childMethod();
  const fromBase = child.baseMethod();
  const method = child.accessorMethod;

  lexicalArrow();
  boundTarget.bind(child)();
  boundTarget.call(child);
  boundTarget.apply(child);

  return fromChild + fromBase + method();
}

prototypeEntry();
