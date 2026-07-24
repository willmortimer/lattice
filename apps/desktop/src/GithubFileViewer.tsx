import type { OpenResourceSession } from "./resourceSession";

type GithubFileSession = Extract<OpenResourceSession, { kind: "github-file" }>;

export function GithubFileViewer({ session }: { session: GithubFileSession }) {
  return (
    <div className="github-file-viewer" aria-label="Read-only GitHub file">
      <div className="github-file-banner">
        <span>
          Read-only · {session.owner}/{session.repo}/{session.path}
        </span>
        {session.stale && <span className="connected-stale">Stale / offline extract</span>}
      </div>
      <pre className="github-file-content">{session.content}</pre>
    </div>
  );
}
