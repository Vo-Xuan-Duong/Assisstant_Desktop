import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const binary = fileURLToPath(new URL("../target/debug/assistant-mcp.exe", import.meta.url));
const child = spawn(binary, [], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
const pending = new Map();
let nextId = 0;
const lines = createInterface({ input: child.stdout });
lines.on("line", (line) => {
  const message = JSON.parse(line);
  const request = pending.get(message.id);
  if (!request) return;
  pending.delete(message.id);
  if (message.error) request.reject(new Error(JSON.stringify(message.error)));
  else request.resolve(message.result);
});
child.stderr.resume();
child.on("error", (error) => { for (const p of pending.values()) p.reject(error); });
child.on("exit", () => {
  for (const p of pending.values()) p.reject(new Error("MCP exited before replying"));
});
function request(method, params) {
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  });
}
const timeout = setTimeout(() => {
  for (const p of pending.values()) p.reject(new Error("MCP smoke test timed out"));
  child.kill();
}, 20_000);
try {
  await request("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "assistant-local-smoke-test", version: "1.0.0" },
  });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
  const catalogue = await request("tools/list", {});
  if (!catalogue.tools.some((tool) => tool.name === "audio_get_volume")) {
    throw new Error("MCP tool catalogue is missing audio_get_volume");
  }
  const result = await request("tools/call", { name: "audio_get_volume", arguments: {} });
  if (result.isError || !result.content?.length) throw new Error("Native audio query failed");
  console.log(`MCP passed: initialized, listed ${catalogue.tools.length} tools, native audio query succeeded.`);
} finally {
  clearTimeout(timeout);
  lines.close();
  child.kill();
}
