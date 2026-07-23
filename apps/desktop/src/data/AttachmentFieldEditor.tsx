import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useState } from "react";

import { NATIVE_DESKTOP_LABEL, nativeOnlyToolbarTooltip } from "./browserDemoHonesty";
import {
  addAttachmentDraftPath,
  attachmentFileName,
  parseAttachmentDraft,
  removeAttachmentDraftPath,
} from "./recordDetail";

interface AttachmentFieldEditorProps {
  value: string;
  onChange: (next: string) => void;
  root?: string;
  packageRelPath?: string;
  /** False in the browser demo fixture where Tauri file commands are unavailable. */
  nativeFileOps?: boolean;
  readOnly: boolean;
  label: string;
}

export function AttachmentFieldEditor({
  value,
  onChange,
  root,
  packageRelPath,
  nativeFileOps = true,
  readOnly,
  label,
}: AttachmentFieldEditorProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const paths = parseAttachmentDraft(value);
  const canMutateFiles =
    !readOnly &&
    nativeFileOps &&
    Boolean(root?.trim()) &&
    Boolean(packageRelPath?.trim());

  const handleAdd = useCallback(async () => {
    if (!canMutateFiles || !root || !packageRelPath) return;
    setError(null);
    const selected = await open({ multiple: false, title: `Attach file to ${label}` });
    if (!selected || Array.isArray(selected)) return;
    setBusy(true);
    try {
      const packagePath = await invoke<string>("add_data_attachment", {
        root,
        relPath: packageRelPath,
        sourcePath: selected,
      });
      onChange(addAttachmentDraftPath(value, packagePath));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [canMutateFiles, label, onChange, packageRelPath, root, value]);

  const handleRemove = useCallback(
    async (path: string) => {
      if (readOnly) return;
      setError(null);
      onChange(removeAttachmentDraftPath(value, path));
      if (!canMutateFiles || !root || !packageRelPath) return;
      try {
        await invoke("remove_data_attachment", {
          root,
          relPath: packageRelPath,
          attachmentPath: path,
        });
      } catch (err) {
        setError(String(err));
      }
    },
    [canMutateFiles, onChange, packageRelPath, readOnly, root, value],
  );

  return (
    <div className="record-detail-attachments" role="group" aria-label={label}>
      {paths.length === 0 ? (
        <p className="record-detail-attachments-empty">No files attached</p>
      ) : (
        <ul className="record-detail-attachments-list">
          {paths.map((path) => (
            <li key={path} className="record-detail-attachments-item">
              <span className="record-detail-attachments-name" title={path}>
                {attachmentFileName(path)}
              </span>
              {!readOnly && (
                <button
                  type="button"
                  className="record-detail-attachments-remove"
                  onClick={() => void handleRemove(path)}
                >
                  Remove
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {!readOnly && (
        <button
          type="button"
          className="record-detail-attachments-add"
          disabled={busy || !canMutateFiles}
          title={canMutateFiles ? undefined : nativeOnlyToolbarTooltip(NATIVE_DESKTOP_LABEL)}
          onClick={() => void handleAdd()}
        >
          {busy ? "Adding…" : "Add file"}
        </button>
      )}
      {error && <span className="record-detail-field-error">{error}</span>}
    </div>
  );
}
