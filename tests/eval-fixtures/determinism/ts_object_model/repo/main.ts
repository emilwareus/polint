function target(): string {
  return "target";
}

function other(): string {
  return "other";
}

export function run(): string {
  const holder = { target, other };
  return holder.target();
}

run();
