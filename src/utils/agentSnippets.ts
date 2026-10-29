/** Ready-to-paste MCP setup for common AI coding agents. */
export type AgentClientId = "claude-code" | "codex" | "cursor" | "vscode" | "claude-desktop";

export interface AgentClientSetup {
  id: AgentClientId;
  label: string;
  /** Where to run or paste the snippet. */
  target: string;
  /** "command" runs in a terminal; "file" goes into a config file. */
  kind: "command" | "file";
  snippet: string;
}

export const AGENT_CLIENT_IDS: AgentClientId[] = [
  "claude-code",
  "codex",
  "cursor",
  "vscode",
  "claude-desktop",
];

const json = (value: unknown) => JSON.stringify(value, null, 2);

export function agentClientSetup(id: AgentClientId, url: string, token: string): AgentClientSetup {
  const bearer = `Bearer ${token}`;
  switch (id) {
    case "claude-code":
      return {
        id,
        label: "Claude Code",
        target: "Terminal",
        kind: "command",
        snippet: `claude mcp add --transport http --scope user champ ${url} --header "Authorization: ${bearer}"`,
      };
    case "codex":
      return {
        id,
        label: "Codex",
        target: "~/.codex/config.toml",
        kind: "file",
        snippet: `[mcp_servers.champ]\nurl = "${url}"\nhttp_headers = { Authorization = "${bearer}" }`,
      };
    case "cursor":
      return {
        id,
        label: "Cursor",
        target: "~/.cursor/mcp.json",
        kind: "file",
        snippet: json({ mcpServers: { champ: { url, headers: { Authorization: bearer } } } }),
      };
    case "vscode":
      return {
        id,
        label: "VS Code",
        target: ".vscode/mcp.json",
        kind: "file",
        snippet: json({
          servers: { champ: { type: "http", url, headers: { Authorization: bearer } } },
        }),
      };
    case "claude-desktop":
      // Claude Desktop config only launches local commands, so bridge with mcp-remote (Node.js).
      return {
        id,
        label: "Claude Desktop",
        target: "claude_desktop_config.json",
        kind: "file",
        snippet: json({
          mcpServers: {
            champ: {
              command: "npx",
              args: ["-y", "mcp-remote", url, "--header", "Authorization:${CHAMP_AUTH}"],
              env: { CHAMP_AUTH: bearer },
            },
          },
        }),
      };
  }
}

/** Hide the token in on-screen previews; the copied text keeps it. */
export function maskToken(snippet: string, token: string): string {
  return token ? snippet.split(token).join("••••••••") : snippet;
}
