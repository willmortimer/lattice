/**
 * Serialized save loop with an edit-generation counter.
 *
 * Prevents the lost-update race where a debounce fires while a save is in
 * flight, returns early, and never requeues edits made after the snapshot.
 */

export type SerializedSaveStatus =
  | "idle"
  | "dirty"
  | "saving"
  | "saved"
  | "conflict"
  | "error";

export interface SerializedSaveControllerOptions<TRevision> {
  /** Persist the current document; returns the new base revision. */
  save: (revision: TRevision) => Promise<TRevision>;
  /** Called when status transitions (or message changes). */
  onStatus: (status: SerializedSaveStatus, message?: string) => void;
  /** Called after a successful save with the new revision. */
  onRevision?: (revision: TRevision) => void;
  /** Classify an error as a conflict (stale revision) vs generic failure. */
  isConflict?: (error: unknown) => boolean;
  /** Extract a user-facing message from a save failure. */
  errorMessage?: (error: unknown) => string;
  /** How long "saved" lingers before returning to idle (0 = stay saved). */
  savedIndicatorMs?: number;
  /** Initial base revision from the load snapshot. */
  initialRevision: TRevision;
}

export interface SerializedSaveController<TRevision = unknown> {
  /** Bump the edit generation and schedule a debounced flush. */
  markDirty: (debounceMs: number) => void;
  /** Flush immediately (manual save, blur, window close). */
  flush: () => Promise<void>;
  /** Cancel a pending debounce timer (does not abort an in-flight save). */
  clearTimer: () => void;
  /** Block further saves until conflict is resolved. */
  setConflict: (message: string) => void;
  /** Reset to idle after reload / adopt; optionally seed a new base revision. */
  reset: (nextRevision?: TRevision) => void;
  /** Whether a conflict is blocking saves. */
  isConflicted: () => boolean;
  /** Dispose timers. */
  dispose: () => void;
}

const defaultIsConflict = (error: unknown): boolean =>
  error instanceof Error && error.name === "StaleRevisionError";

const defaultErrorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Create a serialized save controller. Callers must keep one instance per
 * editor surface and share it across autosave, manual save, and close flush.
 */
export function createSerializedSaveController<TRevision>(
  options: SerializedSaveControllerOptions<TRevision>,
): SerializedSaveController<TRevision> {
  const {
    save,
    onStatus,
    onRevision,
    isConflict = defaultIsConflict,
    errorMessage = defaultErrorMessage,
    savedIndicatorMs = 1500,
    initialRevision,
  } = options;

  let editVersion = 0;
  let savedVersion = 0;
  let revision: TRevision = initialRevision;
  let conflicted = false;
  let flushPromise: Promise<void> | null = null;
  let debounceTimer: number | null = null;
  let savedIndicatorTimer: number | null = null;
  let disposed = false;
  let lastStatus: SerializedSaveStatus = "idle";
  let lastMessage: string | undefined;

  const clearDebounce = () => {
    if (debounceTimer !== null) {
      globalThis.clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  };

  const clearSavedIndicator = () => {
    if (savedIndicatorTimer !== null) {
      globalThis.clearTimeout(savedIndicatorTimer);
      savedIndicatorTimer = null;
    }
  };

  const report = (status: SerializedSaveStatus, message?: string) => {
    if (disposed) return;
    if (status === lastStatus && message === lastMessage) return;
    lastStatus = status;
    lastMessage = message;
    onStatus(status, message);
  };

  const runFlush = async (): Promise<void> => {
    if (conflicted) return;

    while (!disposed && !conflicted && savedVersion !== editVersion) {
      const versionBeingSaved = editVersion;
      report("saving");
      try {
        const nextRevision = await save(revision);
        if (disposed) return;
        revision = nextRevision;
        onRevision?.(nextRevision);
        savedVersion = versionBeingSaved;
      } catch (error) {
        if (disposed) return;
        if (isConflict(error)) {
          conflicted = true;
          report("conflict", errorMessage(error));
          return;
        }
        report("error", errorMessage(error));
        return;
      }
    }

    if (disposed || conflicted) return;
    if (savedVersion === editVersion) {
      report("saved");
      clearSavedIndicator();
      if (savedIndicatorMs > 0) {
        savedIndicatorTimer = globalThis.setTimeout(() => {
          savedIndicatorTimer = null;
          if (!disposed && savedVersion === editVersion && !conflicted) {
            report("idle");
          }
        }, savedIndicatorMs) as unknown as number;
      }
    }
  };

  const flush = (): Promise<void> => {
    clearDebounce();
    if (conflicted) return Promise.resolve();
    if (savedVersion === editVersion && !flushPromise) {
      return Promise.resolve();
    }
    if (flushPromise) return flushPromise;
    flushPromise = runFlush().finally(() => {
      flushPromise = null;
      // Edits that arrived while the last save finished need another pass.
      if (!disposed && !conflicted && savedVersion !== editVersion) {
        void flush();
      }
    });
    return flushPromise;
  };

  return {
    markDirty(debounceMs: number) {
      if (conflicted) return;
      editVersion += 1;
      clearSavedIndicator();
      report("dirty");
      clearDebounce();
      debounceTimer = globalThis.setTimeout(() => {
        debounceTimer = null;
        void flush();
      }, debounceMs) as unknown as number;
    },
    flush,
    clearTimer: clearDebounce,
    setConflict(message: string) {
      conflicted = true;
      clearDebounce();
      report("conflict", message);
    },
    reset(nextRevision) {
      conflicted = false;
      editVersion = 0;
      savedVersion = 0;
      if (nextRevision !== undefined) {
        revision = nextRevision;
      }
      lastStatus = "idle";
      lastMessage = undefined;
      clearDebounce();
      clearSavedIndicator();
      onStatus("idle");
    },
    isConflicted: () => conflicted,
    dispose() {
      disposed = true;
      clearDebounce();
      clearSavedIndicator();
    },
  };
}
