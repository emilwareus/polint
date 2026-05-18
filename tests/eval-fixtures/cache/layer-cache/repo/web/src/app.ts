import { rate } from "@billing/rates";

export const amount = 42;

export function charge(multiplier: number): number {
  return amount * multiplier * rate;
}
