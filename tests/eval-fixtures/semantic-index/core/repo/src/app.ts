import defaultHandler, {
  handler as importedHandler,
  reexportedHandler,
} from "./lib";
import * as namespace from "./lib";

declare const require: (path: string) => unknown;

export { reexportedHandler as reexport };

// dynamic import
const dynamicPath = "./lib";
const dynamicModulePromise = import(dynamicPath);

export async function run() {
  const cjsModule = require("./lib");
  const namespaceValue = namespace.handler;
  const missing = missingSymbol;

  function shadow() {
    const handler = importedHandler;
    return handler;
  }

  return [
    defaultHandler,
    importedHandler,
    reexportedHandler,
    await dynamicModulePromise,
    cjsModule,
    namespaceValue,
    missing,
    shadow(),
  ];
}
