import { type ComponentType, type ReactNode, useMemo } from "react";
import { type ToolCallMessagePartProps } from "@assistant-ui/react";

export type SemanticToolRenderer = {
  toolNames: string[];
  display: "inline" | "standalone";
  render: ComponentType<ToolCallMessagePartProps>;
};

export type SemanticToolArg = {
  label: string;
  value: string;
};

export function toolStatusLabel(status: ToolCallMessagePartProps["status"]): string {
  switch (status.type) {
    case "running":
      return "Running";
    case "complete":
      return "Done";
    case "incomplete":
      return "Failed";
    case "requires-action":
      return "Waiting";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

function toolStatusDetail(status: ToolCallMessagePartProps["status"]): string | null {
  if (status.type === "incomplete" && status.reason) {
    return status.reason;
  }
  if (status.type === "requires-action" && status.reason) {
    return status.reason;
  }
  return null;
}

function argValue(args: unknown, key: string): unknown {
  if (!args || typeof args !== "object") return undefined;
  return (args as Record<string, unknown>)[key];
}

function argString(args: unknown, key: string): string | null {
  const value = argValue(args, key);
  if (typeof value === "string" && value.trim().length > 0) return value;
  return null;
}

function formatArgValue(value: unknown): string | null {
  if (typeof value === "string" && value.trim().length > 0) return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (Array.isArray(value) && value.length > 0) {
    return value.map(String).join(" ");
  }
  return null;
}

type ArgSpec = {
  key: string;
  label: string;
  format?: (value: unknown) => string | null;
};

function buildArgRows(args: unknown, specs: ArgSpec[]): SemanticToolArg[] {
  const rows: SemanticToolArg[] = [];
  for (const spec of specs) {
    const raw = argValue(args, spec.key);
    if (raw === undefined || raw === null) continue;
    const formatted = spec.format ? spec.format(raw) : formatArgValue(raw);
    if (formatted) {
      rows.push({ label: spec.label, value: formatted });
    }
  }
  return rows;
}

function formatByteRange(args: unknown): string | null {
  const start = argValue(args, "startByte");
  const end = argValue(args, "endByte");
  const maxBytes = argValue(args, "maxBytes");
  if (typeof start === "number" && typeof end === "number") {
    return `${start}–${end}`;
  }
  if (typeof maxBytes === "number") {
    return `max ${maxBytes} bytes`;
  }
  return null;
}

function formatJsonArrayPreview(value: unknown): string | null {
  if (typeof value !== "string" || value.trim().length === 0) return null;
  try {
    const parsed = JSON.parse(value) as unknown;
    if (Array.isArray(parsed)) {
      return `${parsed.length} item${parsed.length === 1 ? "" : "s"}`;
    }
    if (parsed && typeof parsed === "object") {
      return `${Object.keys(parsed as Record<string, unknown>).length} fields`;
    }
  } catch {
    return value.length > 80 ? `${value.slice(0, 77)}…` : value;
  }
  return null;
}

function formatToolPayload(args: unknown, argsText: string | undefined): string {
  if (argsText && argsText.trim().length > 0) return argsText;
  try {
    return JSON.stringify(args ?? {}, null, 2);
  } catch {
    return String(args);
  }
}

type SemanticToolShellProps = {
  display: SemanticToolRenderer["display"];
  label: string;
  detail?: string | null;
  status: ToolCallMessagePartProps["status"];
  argRows?: SemanticToolArg[];
  rawArgs: unknown;
  argsText?: string;
  children?: ReactNode;
};

function SemanticToolShell({
  display,
  label,
  detail,
  status,
  argRows,
  rawArgs,
  argsText,
  children,
}: SemanticToolShellProps) {
  const payload = useMemo(() => formatToolPayload(rawArgs, argsText), [rawArgs, argsText]);
  const statusDetail = toolStatusDetail(status);

  return (
    <div
      className={`agent-semantic-tool agent-semantic-tool--${display}`}
      data-tool-status={status.type}
    >
      <div className="agent-semantic-tool-head">
        <span className="agent-semantic-tool-label">{label}</span>
        {detail ? <span className="agent-semantic-tool-detail">{detail}</span> : null}
        <span className="agent-semantic-tool-status">{toolStatusLabel(status)}</span>
      </div>
      {statusDetail ? (
        <p className="agent-semantic-tool-status-detail">{statusDetail}</p>
      ) : null}
      {argRows && argRows.length > 0 ? (
        <dl className="agent-semantic-tool-args">
          {argRows.map((row) => (
            <div key={row.label} className="agent-semantic-tool-arg">
              <dt>{row.label}</dt>
              <dd>{row.value}</dd>
            </div>
          ))}
        </dl>
      ) : null}
      {children}
      <details className="agent-semantic-tool-raw">
        <summary className="agent-semantic-tool-raw-summary">Raw JSON</summary>
        <pre className="agent-semantic-tool-raw-body">{payload}</pre>
      </details>
    </div>
  );
}

function shellProps(
  props: ToolCallMessagePartProps,
  shell: Omit<SemanticToolShellProps, "rawArgs" | "argsText" | "status">
): SemanticToolShellProps {
  return {
    ...shell,
    status: props.status,
    rawArgs: props.args,
    argsText: props.argsText,
  };
}

function SearchToolSurface(props: ToolCallMessagePartProps) {
  const query = argString(props.args, "query");
  const argRows = buildArgRows(props.args, [
    { key: "query", label: "Query" },
    { key: "limit", label: "Limit" },
  ]);
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "inline",
        label: "Search",
        detail: query,
        argRows,
      })}
    />
  );
}

function ReadResourceToolSurface(props: ToolCallMessagePartProps) {
  const path = argString(props.args, "path");
  const argRows = buildArgRows(props.args, [{ key: "path", label: "Path" }]);
  const byteRange = formatByteRange(props.args);
  if (byteRange) {
    argRows.push({ label: "Range", value: byteRange });
  }
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "inline",
        label: "Read",
        detail: path,
        argRows,
      })}
    />
  );
}

function CreateProposalToolSurface(props: ToolCallMessagePartProps) {
  const summary =
    argString(props.args, "summary") ??
    argString(props.args, "path") ??
    argString(props.args, "proposalId");
  const argRows = buildArgRows(props.args, [
    { key: "summary", label: "Summary" },
    { key: "sourceResource", label: "Source" },
    { key: "commandsJson", label: "Commands", format: formatJsonArrayPreview },
    { key: "affectedPathsJson", label: "Affected paths", format: formatJsonArrayPreview },
    { key: "warningsJson", label: "Warnings", format: formatJsonArrayPreview },
  ]);
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "standalone",
        label: "Create proposal",
        detail: summary,
        argRows,
      })}
    />
  );
}

function ApplyProposalToolSurface(props: ToolCallMessagePartProps) {
  const proposalId =
    argString(props.args, "proposalId") ?? argString(props.args, "id") ?? argString(props.args, "path");
  const argRows = buildArgRows(props.args, [
    { key: "proposalId", label: "Proposal" },
    { key: "id", label: "Id" },
    { key: "path", label: "Path" },
  ]);
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "standalone",
        label: "Apply proposal",
        detail: proposalId,
        argRows,
      })}
    />
  );
}

function RunCommandToolSurface(props: ToolCallMessagePartProps) {
  const detail =
    argString(props.args, "cellId") ??
    argString(props.args, "projectionId") ??
    argString(props.args, "wasmPath") ??
    argString(props.args, "preset") ??
    argString(props.args, "command");
  const argRows = buildArgRows(props.args, [
    { key: "cellId", label: "Cell" },
    { key: "projectionId", label: "Projection" },
    { key: "outputProposalTarget", label: "Output target" },
    { key: "argv", label: "Argv" },
    { key: "profile", label: "Profile" },
    { key: "executionMode", label: "Mode" },
    { key: "command", label: "Command" },
    { key: "wasmPath", label: "Wasm" },
    { key: "preset", label: "Preset" },
  ]);
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "standalone",
        label: "Run",
        detail,
        argRows,
      })}
    />
  );
}

function ApprovalToolSurface(props: ToolCallMessagePartProps) {
  const detail =
    argString(props.args, "message") ??
    argString(props.args, "reason") ??
    argString(props.args, "action");
  const awaitingApproval = props.status.type === "requires-action";
  const argRows = buildArgRows(props.args, [
    { key: "action", label: "Action" },
    { key: "message", label: "Message" },
    { key: "reason", label: "Reason" },
  ]);
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "standalone",
        label: awaitingApproval ? "Approval required" : "Approval",
        detail,
        argRows,
      })}
    />
  );
}

function MemoryToolSurface(props: ToolCallMessagePartProps) {
  const isRecall = props.toolName === "recall";
  const detail =
    argString(props.args, isRecall ? "query" : "text") ?? argString(props.args, "id");
  const argRows = buildArgRows(props.args, [
    { key: isRecall ? "query" : "text", label: isRecall ? "Query" : "Text" },
    { key: "id", label: "Memory id" },
    { key: "limit", label: "Limit" },
  ]);
  return (
    <SemanticToolShell
      {...shellProps(props, {
        display: "inline",
        label: isRecall ? "Recall memory" : "Remember",
        detail,
        argRows,
      })}
    />
  );
}

export const semanticToolRenderers: SemanticToolRenderer[] = [
  {
    toolNames: ["search", "workspace.search"],
    display: "inline",
    render: SearchToolSurface,
  },
  {
    toolNames: ["read", "read_resource", "workspace.read"],
    display: "inline",
    render: ReadResourceToolSurface,
  },
  {
    toolNames: [
      "create_proposal",
      "workspace.proposal.create",
      "propose_page",
      "propose_resource",
      "propose_workflow",
      "propose_interface",
      "propose_artifact",
      "workspace.proposal.propose_page",
      "workspace.proposal.propose_resource",
      "workspace.proposal.propose_workflow",
      "workspace.proposal.propose_interface",
      "workspace.proposal.propose_artifact",
    ],
    display: "standalone",
    render: CreateProposalToolSurface,
  },
  {
    toolNames: ["apply_proposal", "workspace.proposal.apply"],
    display: "standalone",
    render: ApplyProposalToolSurface,
  },
  {
    toolNames: ["run_cell_task", "run_wasi_guest", "run_command", "run_cell"],
    display: "standalone",
    render: RunCommandToolSurface,
  },
  {
    toolNames: ["approval", "request_approval", "confirm_action"],
    display: "standalone",
    render: ApprovalToolSurface,
  },
  {
    toolNames: ["remember", "recall"],
    display: "inline",
    render: MemoryToolSurface,
  },
];

export function resolveSemanticToolRenderer(toolName: string): SemanticToolRenderer | null {
  for (const entry of semanticToolRenderers) {
    if (entry.toolNames.includes(toolName)) return entry;
  }
  return null;
}

export function buildSemanticToolComponentsByName(): Record<
  string,
  ComponentType<ToolCallMessagePartProps>
> {
  const byName: Record<string, ComponentType<ToolCallMessagePartProps>> = {};
  for (const entry of semanticToolRenderers) {
    for (const toolName of entry.toolNames) {
      byName[toolName] = entry.render;
    }
  }
  return byName;
}

export function RawJsonToolFallback({ toolName, status, args, argsText }: ToolCallMessagePartProps) {
  const payload = useMemo(() => formatToolPayload(args, argsText), [args, argsText]);
  return (
    <details className="agent-tool-raw-json">
      <summary className="agent-tool-raw-json-summary">
        <span className="agent-tool-raw-json-label">Tool</span>
        <code>{toolName}</code>
        <span className="agent-tool-raw-json-status">{toolStatusLabel(status)}</span>
      </summary>
      <pre className="agent-tool-raw-json-body">{payload}</pre>
    </details>
  );
}

export function SemanticToolCall(props: ToolCallMessagePartProps) {
  const entry = resolveSemanticToolRenderer(props.toolName);
  if (!entry) {
    return <RawJsonToolFallback {...props} />;
  }
  const Render = entry.render;
  return <Render {...props} />;
}
