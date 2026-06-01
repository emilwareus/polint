import defaultTarget from "./default";
import * as ns from "./namespace";
import { namedTarget as namedAlias } from "./named";
import { viaBarrel } from "./barrel";
import { aliasTarget } from "@lib/target";

const cjsMember = require("./cjs").cjsTarget;
const localAlias = localTarget;
const { destructured } = { destructured: localTarget };

function localTarget(value: string): string {
  return value;
}

export function entry(input: string): string {
  localAlias(input);
  namedAlias(input);
  defaultTarget(input);
  ns.namespaceTarget(input);
  viaBarrel(input);
  aliasTarget(input);
  cjsMember(input);
  destructured(input);
  import(input);
  return input;
}
