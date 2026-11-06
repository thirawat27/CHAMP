import { describe, expect, it } from "vitest";
import { AGENT_CLIENT_IDS, agentClientSetup, maskToken } from "./agentSnippets";

const URL = "http://127.0.0.1:47823/mcp";
const TOKEN = "abc123";

describe("agentClientSetup", () => {
  it("includes the endpoint for every client", () => {
    for (const id of AGENT_CLIENT_IDS) {
      expect(agentClientSetup(id, URL, TOKEN).snippet).toContain(URL);
    }
  });

  it("includes the token for clients that do not prompt for it", () => {
    const clients = AGENT_CLIENT_IDS.filter(
      (id) => !agentClientSetup(id, URL, TOKEN).promptsForToken
    );
    expect(clients).toEqual(["claude-code", "codex", "cursor", "claude-desktop"]);
    for (const id of clients) {
      expect(agentClientSetup(id, URL, TOKEN).snippet).toContain(`Bearer ${TOKEN}`);
    }
  });

  it("keeps the token out of the commonly committed VS Code config", () => {
    const setup = agentClientSetup("vscode", URL, TOKEN);
    const config = JSON.parse(setup.snippet);
    expect(setup.promptsForToken).toBe(true);
    expect(config.inputs[0]).toMatchObject({ id: "champ-token", password: true });
    expect(config.servers.champ.headers.Authorization).toBe("Bearer ${input:champ-token}");
  });

  it("pins the mcp-remote bridge to an exact version", () => {
    const args: string[] = JSON.parse(agentClientSetup("claude-desktop", URL, TOKEN).snippet)
      .mcpServers.champ.args;
    expect(args.find((arg) => arg.startsWith("mcp-remote"))).toMatch(/^mcp-remote@\d+\.\d+\.\d+$/);
  });

  it("adds Claude Code at user scope so every project sees CHAMP", () => {
    expect(agentClientSetup("claude-code", URL, TOKEN).snippet).toBe(
      `claude mcp add --transport http --scope user champ ${URL} --header "Authorization: Bearer ${TOKEN}"`
    );
  });

  it("produces valid JSON for JSON-based clients", () => {
    for (const id of ["cursor", "vscode", "claude-desktop"] as const) {
      expect(() => JSON.parse(agentClientSetup(id, URL, TOKEN).snippet)).not.toThrow();
    }
    expect(JSON.parse(agentClientSetup("vscode", URL, TOKEN).snippet).servers.champ.type).toBe(
      "http"
    );
  });

  it("masks every occurrence of the token", () => {
    const masked = maskToken(agentClientSetup("claude-desktop", URL, TOKEN).snippet, TOKEN);
    expect(masked).not.toContain(TOKEN);
    expect(maskToken("no token", "")).toBe("no token");
  });
});
