export const handler = "native-handler";

const defaultHandler = () => handler;
export default defaultHandler;

export const reexportValue = handler;
export { handler as reexportedHandler };
