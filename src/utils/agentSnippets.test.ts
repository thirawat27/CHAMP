import { describe, expect, it } from "vitest";
import { AGENT_CLIENT_IDS, agentClientSetup, maskToken } from "./agentSnippets";

const URL = "http://127.0.0.1:47823/mcp";
const TOKEN = "abc123";

describe("agentClientSetup", () => {
  it("includes the endpoint and token for every client", () => {
    for (const id of AGENT_CLIENT_IDS) {
      const setup = agentClientSetup(id, URL, TOKEN);
      expect(setup.snippet).toContain(URL);
      expect(setup.snippet).toContain(`Bearer ${TOKEN}`);
    }
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
