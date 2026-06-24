// A cross-file consumer of the exported `formatUser` symbol from src/api.ts.
import { formatUser } from "./api";

export function greet(name: string): string {
  return `Hello, ${formatUser(name)}`;
}
