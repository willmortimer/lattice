import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import { presentAuthorizeUrl } from "../lib/authPresenter";
import {
  cloudBeginBrowserSiwa,
  cloudSignIn,
  cloudSignInApple,
  type CloudSessionStatus,
} from "../lib/cloud";
import {
  formatEncryptedBackupOption,
  type AccountCloudWorkspace,
  type EncryptedBackupListEntry,
} from "../lib/encryptedBackup";
import { hasTauri } from "../lib/ipc";
import {
  setCloudSessionCache,
  useCloudSessionQuery,
} from "../query/useCloudSessionQuery";
import {
  defaultEmptyRestoreCommands,
  emptyCloudRestorePhase,
  loadEmptyRestoreBackups,
  loadEmptyRestoreWorkspaces,
  nextSelectedId,
  runEmptyShellRestore,
  supportsNativeAppleSignIn,
  type EmptyCloudRestorePhase,
} from "./emptyCloudRestore";

export interface EmptyCloudRestorePanelProps {
  panel: "sign-in" | "restore";
  shellBusy: boolean;
  onOpenRestoredWorkspace: (path: string) => Promise<void>;
}

function phaseCopy(phase: EmptyCloudRestorePhase): string {
  switch (phase) {
    case "sign-in":
      return "Sign in to restore an encrypted backup from Lattice Cloud.";
    case "loading-workspaces":
      return "Loading cloud workspaces…";
    case "pick-workspace":
      return "Choose a cloud workspace, then a backup and a destination folder.";
    case "loading-backups":
      return "Loading backups…";
    case "pick-backup":
      return "Choose a backup and a folder to restore into, then Restore.";
    case "restoring":
      return "Restoring encrypted backup…";
    default: {
      const unreachable: never = phase;
      return unreachable;
    }
  }
}

export function EmptyCloudRestorePanel({
  panel,
  shellBusy,
  onOpenRestoredWorkspace,
}: EmptyCloudRestorePanelProps) {
  const queryClient = useQueryClient();
  const { data: status = null } = useCloudSessionQuery();
  const signedIn = Boolean(status?.signedIn);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [signInBusy, setSignInBusy] = useState(false);
  const [signInError, setSignInError] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<AccountCloudWorkspace[]>([]);
  const [selectedCloudWorkspaceId, setSelectedCloudWorkspaceId] = useState("");
  const [backups, setBackups] = useState<EncryptedBackupListEntry[]>([]);
  const [selectedBackupId, setSelectedBackupId] = useState("");
  const [restoreTarget, setRestoreTarget] = useState<string | null>(null);
  const [workspacesLoading, setWorkspacesLoading] = useState(false);
  const [backupsLoading, setBackupsLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [restoreBusy, setRestoreBusy] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);

  const phase = emptyCloudRestorePhase({
    signedIn,
    workspacesLoading,
    selectedCloudWorkspaceId,
    backupsLoading,
    restoring: restoreBusy,
  });
  const controlsDisabled = shellBusy || signInBusy || restoreBusy || inBrowser;
  const showNativeApple = supportsNativeAppleSignIn();

  useEffect(() => {
    if (status?.user?.email) {
      setEmail(status.user.email);
    }
  }, [status?.user?.email]);

  useEffect(() => {
    if (inBrowser || !hasTauri) return;
    let cancelled = false;
    const unlistenSession = listen<CloudSessionStatus>("cloud-session-changed", (event) => {
      if (cancelled) return;
      setCloudSessionCache(queryClient, event.payload);
      if (event.payload.user?.email) setEmail(event.payload.user.email);
      if (event.payload.error) setSignInError(event.payload.error);
      setSignInBusy(false);
    });
    const unlistenError = listen<string>("cloud-sign-in-error", (event) => {
      if (cancelled) return;
      setSignInError(event.payload);
      setSignInBusy(false);
    });
    return () => {
      cancelled = true;
      void unlistenSession.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  }, [queryClient]);

  useEffect(() => {
    if (inBrowser || !signedIn) {
      setWorkspaces([]);
      setSelectedCloudWorkspaceId("");
      setListError(null);
      setWorkspacesLoading(false);
      return;
    }
    let cancelled = false;
    setWorkspacesLoading(true);
    setListError(null);
    void loadEmptyRestoreWorkspaces(defaultEmptyRestoreCommands.listWorkspaces).then((loaded) => {
      if (cancelled) return;
      if (!loaded.ok) {
        setWorkspaces([]);
        setSelectedCloudWorkspaceId("");
        setListError(loaded.error);
        setWorkspacesLoading(false);
        return;
      }
      setWorkspaces(loaded.workspaces);
      setSelectedCloudWorkspaceId((prev) =>
        nextSelectedId(
          loaded.workspaces.map((workspace) => workspace.id),
          prev,
        ),
      );
      setWorkspacesLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, [signedIn]);

  useEffect(() => {
    if (inBrowser || !signedIn || !selectedCloudWorkspaceId) {
      setBackups([]);
      setSelectedBackupId("");
      setBackupsLoading(false);
      return;
    }
    let cancelled = false;
    setBackupsLoading(true);
    setListError(null);
    void loadEmptyRestoreBackups(
      selectedCloudWorkspaceId,
      defaultEmptyRestoreCommands.listBackups,
    ).then((loaded) => {
      if (cancelled) return;
      if (!loaded.ok) {
        setBackups([]);
        setSelectedBackupId("");
        setListError(loaded.error);
        setBackupsLoading(false);
        return;
      }
      setBackups(loaded.backups);
      setSelectedBackupId((prev) => nextSelectedId(loaded.backups.map((backup) => backup.id), prev));
      setBackupsLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, [signedIn, selectedCloudWorkspaceId]);

  async function handlePasswordSignIn() {
    if (inBrowser) return;
    setSignInBusy(true);
    setSignInError(null);
    try {
      const next = await cloudSignIn(email.trim(), password);
      setCloudSessionCache(queryClient, next);
      setPassword("");
      if (next.error) setSignInError(next.error);
    } catch (err: unknown) {
      setSignInError(err instanceof Error ? err.message : String(err));
    } finally {
      setSignInBusy(false);
    }
  }

  async function handleAppleSignIn() {
    if (inBrowser) return;
    setSignInBusy(true);
    setSignInError(null);
    try {
      const next = await cloudSignInApple();
      setCloudSessionCache(queryClient, next);
      if (next.user?.email) setEmail(next.user.email);
      if (next.error) setSignInError(next.error);
    } catch (err: unknown) {
      setSignInError(err instanceof Error ? err.message : String(err));
    } finally {
      setSignInBusy(false);
    }
  }

  async function handleBrowserAppleSignIn() {
    if (inBrowser) return;
    setSignInBusy(true);
    setSignInError(null);
    try {
      const authorizeUrl = await cloudBeginBrowserSiwa();
      await presentAuthorizeUrl(authorizeUrl);
    } catch (err: unknown) {
      setSignInError(err instanceof Error ? err.message : String(err));
      setSignInBusy(false);
    }
  }

  async function handleChooseRestoreFolder() {
    if (inBrowser) return;
    const path = await open({
      directory: true,
      multiple: false,
      title: "Choose restore destination",
    });
    if (typeof path === "string") setRestoreTarget(path);
  }

  async function handleRestore() {
    if (inBrowser || restoreBusy) return;
    setRestoreBusy(true);
    setRestoreError(null);
    try {
      const outcome = await runEmptyShellRestore(
        {
          cloudWorkspaceId: selectedCloudWorkspaceId,
          backupId: selectedBackupId,
          targetRoot: restoreTarget,
        },
        {
          restore: defaultEmptyRestoreCommands.restore,
          openWorkspace: onOpenRestoredWorkspace,
        },
      );
      if (!outcome.ok) {
        setRestoreError(outcome.error);
      }
    } finally {
      setRestoreBusy(false);
    }
  }

  const showSignIn = !signedIn;
  const showRestore = signedIn;

  if (inBrowser) {
    return (
      <div className="empty-restore-panel" role="region" aria-label="Restore encrypted backup">
        <p className="empty-restore-copy">
          Sign in and restore require the Lattice desktop app.
        </p>
      </div>
    );
  }

  return (
    <div
      className="empty-restore-panel"
      role="region"
      aria-label={panel === "sign-in" ? "Sign in to Lattice Cloud" : "Restore encrypted backup"}
    >
      <p className="empty-restore-copy">{phaseCopy(phase)}</p>
      {status?.signedIn ? (
        <p className="empty-restore-identity">
          Signed in as{" "}
          {status.user?.email ?? status.user?.display_name ?? "your Lattice Cloud account"}
        </p>
      ) : null}

      {showSignIn ? (
        <div className="cloud-signin-panel">
          <div className="cloud-signin-primary">
            <button
              type="button"
              className="secondary-button empty-restore-button"
              disabled={controlsDisabled}
              onClick={() => void handleBrowserAppleSignIn()}
            >
              {signInBusy ? "Waiting for browser…" : "Continue with Apple"}
            </button>
            {showNativeApple ? (
              <button
                type="button"
                className="secondary-button empty-restore-button"
                disabled={controlsDisabled}
                onClick={() => void handleAppleSignIn()}
              >
                {signInBusy ? "Signing in…" : "Continue with Apple (native)"}
              </button>
            ) : null}
          </div>
          <div className="cloud-signin-divider" role="presentation">
            <span>or password</span>
          </div>
          <div className="cloud-signin-password">
            <label className="cloud-signin-field">
              <span>Email</span>
              <input
                type="email"
                autoComplete="username"
                value={email}
                disabled={controlsDisabled}
                onChange={(event) => setEmail(event.currentTarget.value)}
              />
            </label>
            <label className="cloud-signin-field">
              <span>Password</span>
              <input
                type="password"
                autoComplete="current-password"
                value={password}
                disabled={controlsDisabled}
                onChange={(event) => setPassword(event.currentTarget.value)}
              />
            </label>
            <button
              type="button"
              className="secondary-button empty-restore-button"
              disabled={controlsDisabled || !email.trim() || !password}
              onClick={() => void handlePasswordSignIn()}
            >
              {signInBusy ? "Signing in…" : "Sign in with password"}
            </button>
          </div>
        </div>
      ) : null}

      {signInError ? (
        <p className="error-text" role="alert">
          {signInError}
        </p>
      ) : null}

      {showRestore ? (
        <div className="cloud-signin-password">
          <label className="cloud-signin-field">
            <span>Cloud workspace</span>
            <select
              value={selectedCloudWorkspaceId}
              disabled={controlsDisabled || workspacesLoading || workspaces.length === 0}
              onChange={(event) => setSelectedCloudWorkspaceId(event.currentTarget.value)}
            >
              {workspaces.length === 0 ? (
                <option value="">
                  {workspacesLoading ? "Loading workspaces…" : "No cloud workspaces"}
                </option>
              ) : (
                workspaces.map((workspace) => (
                  <option key={workspace.id} value={workspace.id}>
                    {workspace.name || workspace.id}
                  </option>
                ))
              )}
            </select>
          </label>
          <label className="cloud-signin-field">
            <span>Backup</span>
            <select
              value={selectedBackupId}
              disabled={
                controlsDisabled ||
                backupsLoading ||
                backups.length === 0 ||
                !selectedCloudWorkspaceId
              }
              onChange={(event) => setSelectedBackupId(event.currentTarget.value)}
            >
              {backups.length === 0 ? (
                <option value="">{backupsLoading ? "Loading backups…" : "No backups yet"}</option>
              ) : (
                backups.map((backup) => (
                  <option key={backup.id} value={backup.id}>
                    {formatEncryptedBackupOption(backup)}
                  </option>
                ))
              )}
            </select>
          </label>
          <label className="cloud-signin-field">
            <span>Restore destination</span>
            <span className="empty-restore-path">{restoreTarget ?? "Choose a folder"}</span>
          </label>
          <div className="empty-restore-actions">
            <button
              type="button"
              className="secondary-button empty-restore-button"
              disabled={controlsDisabled}
              onClick={() => void handleChooseRestoreFolder()}
            >
              Choose restore folder
            </button>
            <button
              type="button"
              className="primary-button empty-restore-button"
              disabled={
                controlsDisabled ||
                !selectedCloudWorkspaceId ||
                !selectedBackupId ||
                !restoreTarget
              }
              onClick={() => void handleRestore()}
            >
              {restoreBusy ? "Restoring…" : "Restore backup"}
            </button>
          </div>
        </div>
      ) : null}

      {listError ? (
        <p className="error-text" role="alert">
          {listError}
        </p>
      ) : null}
      {restoreError ? (
        <p className="error-text" role="alert">
          {restoreError}
        </p>
      ) : null}
    </div>
  );
}
