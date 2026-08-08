import { helper } from "./util";
import { Widget } from "./widget";

const TOKEN_NAME = "api-token";

export function dangerous(): void {}
export function writeBalance(): void {}
export function authorize(): void {}
export function Begin(): { id: number } {
  return { id: 1 };
}
export function Rollback(): void {}

export function Build(input: string): string {
  const local = input;
  if (local === "") {
    return "empty";
  }
  return local;
}

export function handler(token: string): void {
  dangerous();
  writeBalance();
  const tx = Begin();
  void tx;
  console.log(token);
  void helper("matrix-literal");
  void new Widget().label();
}

export function main(): void {
  const w = new Widget();
  const local = Build(w.label());
  handler(local + TOKEN_NAME);
}
