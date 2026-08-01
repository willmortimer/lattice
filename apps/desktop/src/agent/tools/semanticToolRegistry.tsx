import { type ComponentType, type ReactNode, useMemo } from "react";
import { type ToolCallMessagePartProps } from "@assistant-ui/react";

export type SemanticToolRenderer = {
  toolNames: string[];
  display: "inline" | "standalone";
  render: ComponentType<ToolCallMessagePartProps>;
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

function argString(args: unknown, key: string): string | null {
  if (!args || typeof args !== "object") return null;
  const value = (args as Record<string, unknown>)[key];
  if (typeof value === "string" && value.trim().length > 0) return value;
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
  children?: ReactNode;
};

function SemanticToolShell({ display, label, detail, status, children }: SemanticToolShellProps) {
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
      {children}
    </div>
  );
}

function SearchToolSurface(props: ToolCallMessagePartProps) {
  const query = argString(props.args, "query");
  return (
    <SemanticToolShell display="inline" label="Search" detail={query} status={props.status} />
  );
}

function ReadResourceToolSurface(props: ToolCallMessagePartProps) {
  const path = argString(props.args, "path");
  return (
    <SemanticToolShell display="inline" label="Read" detail={path} status={props.status} />
  );
}

function CreateProposalToolSurface(props: ToolCallMessagePartProps) {
  const summary =
    argString(props.args, "summary") ??
    argString(props.args, "path") ??
    argString(props.args, "proposalId");
  return (
    <SemanticToolShell
      display="standalone"
      label="Create proposal"
      detail={summary}
      status={props.status}
    />
  );
}

function ApplyProposalToolSurface(props: ToolCallMessagePartProps) {
  const proposalId =
    argString(props.args, "proposalId") ?? argString(props.args, "id") ?? argString(props.args, "path");
  return (
    <SemanticToolShell
      display="standalone"
      label="Apply proposal"
      detail={proposalId}
      status={props.status}
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
  return (
    <SemanticToolShell display="standalone" label="Run" detail={detail} status={props.status} />
  );
}

function ApprovalToolSurface(props: ToolCallMessagePartProps) {
  const detail =
    argString(props.args, "message") ??
    argString(props.args, "reason") ??
    argString(props.args, "action");
  const awaitingApproval = props.status.type === "requires-action";
  return (
    <SemanticToolShell
      display="standalone"
      label={awaitingApproval ? "Approval required" : "Approval"}
      detail={detail}
      status={props.status}
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
