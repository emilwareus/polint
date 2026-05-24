import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";

const server = new McpServer({
  name: "fixture-mcp-server",
  version: "1.0.0",
});

function calculateHandler(args: { expression: string }): { result: number } {
  return { result: eval(args.expression) };
}

function settingsHandler(uri: string): { settings: Record<string, string> } {
  return { settings: { theme: "dark", language: "en" } };
}

function summarizeHandler(args: { text: string }): { summary: string } {
  return { summary: args.text.slice(0, 100) };
}

server.tool("calculate", calculateHandler);
server.resource("config://settings", settingsHandler);
server.prompt("summarize", summarizeHandler);
