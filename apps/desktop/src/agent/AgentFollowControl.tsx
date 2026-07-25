import { useAgentSessionStore } from "./agentStore";

export function AgentFollowControl() {
  const followMode = useAgentSessionStore((state) => state.followMode);
  const setFollowMode = useAgentSessionStore((state) => state.setFollowMode);

  return (
    <div className="agent-follow-control" role="group" aria-label="Agent follow mode">
      <button
        type="button"
        className={
          followMode === "guide"
            ? "agent-follow-option agent-follow-option-active"
            : "agent-follow-option"
        }
        aria-pressed={followMode === "guide"}
        onClick={() => setFollowMode("guide")}
      >
        Guide
      </button>
      <button
        type="button"
        className={
          followMode === "quiet"
            ? "agent-follow-option agent-follow-option-active"
            : "agent-follow-option"
        }
        aria-pressed={followMode === "quiet"}
        onClick={() => setFollowMode("quiet")}
      >
        Quiet
      </button>
    </div>
  );
}
