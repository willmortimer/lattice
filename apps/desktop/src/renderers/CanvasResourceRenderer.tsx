import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Doc } from "yjs";

import { CanvasViewer } from "../canvas/CanvasViewer";
import {
  createNativeCanvasFileIO,
  isCanvasMaterializeConflict,
  materializeCollabCanvas,
  shouldPatchPlainCanvas,
  shouldScheduleCollabCheckpoint,
} from "../canvas/collab/canvasMaterialize";
import { createCollabCanvasAdapter } from "../canvas/collab/collabCanvasAdapter";
import {
  applyCanvasDataToYDoc,
  CANVAS_SEED_ORIGIN,
  canvasYDocIsEmpty,
} from "../canvas/collab/canvasYDoc";
import { registerCanvasSurface } from "../canvas/registration";
import { parseCanvas } from "../canvas/types";
import type { PagePersistMode } from "../editor/collab/collabSession";
import {
  openCollabSession,
  type CollabSessionHandle,
} from "../editor/collab/collabSession";
import { createSerializedSaveController } from "../editor/serializedSave";
import type { CatalogEntry } from "../lib/resourceCatalog";
import {
  looksLikeLatticeResourceId,
  resourceIdForPath,
} from "../lib/resourceCatalog";
import {
  getResourceStat,
  persistModeFromResourceStat,
  resourceAuthorityForPersistMode,
  setResourceAuthority,
} from "../lib/resourceStat";
import { useCloudSessionQuery } from "../query/useCloudSessionQuery";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

function resolveRegistryResourceId(
  catalog: ReadonlyMap<string, CatalogEntry>,
  path: string,
): string | undefined {
  const catalogId = resourceIdForPath(catalog, path);
  if (catalogId && looksLikeLatticeResourceId(catalogId)) {
    return catalogId;
  }
  return undefined;
}

export function CanvasResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "canvas") return null;
  const { adapter: nativeAdapter } = registerCanvasSurface(
    context.workspaceRoot,
    session.resource.path,
  );
  return (
    <CollaborativeCanvasSurface
      context={context}
      canvasPath={session.resource.path}
      json={session.json}
      revision={session.revision}
      nativeAdapter={nativeAdapter}
    />
  );
}

function CollaborativeCanvasSurface({
  context,
  canvasPath,
  json,
  revision,
  nativeAdapter,
}: {
  context: ResourceRendererContext;
  canvasPath: string;
  json: unknown;
  revision: string;
  nativeAdapter: ReturnType<typeof registerCanvasSurface>["adapter"];
}) {
  const { callbacks, settings } = context;
  const { data: cloudSession } = useCloudSessionQuery();

  const [registryResourceId, setRegistryResourceId] = useState<string | undefined>(() =>
    resolveRegistryResourceId(context.catalog, canvasPath),
  );
  const [persistMode, setPersistMode] = useState<PagePersistMode>("plain");
  const [collabYdoc, setCollabYdoc] = useState<Doc | null>(null);
  const [collabLoading, setCollabLoading] = useState(false);
  const [collabError, setCollabError] = useState<string | null>(null);
  const collabHandleRef = useRef<CollabSessionHandle | null>(null);
  const revisionRef = useRef(revision);
  revisionRef.current = revision;

  const handlePersistModeChange = useCallback(
    async (mode: PagePersistMode) => {
      if (!context.workspaceRoot) {
        console.error("Cannot persist canvas mode without a workspace root");
        return;
      }
      if (mode === "collaborative" && !registryResourceId) {
        console.error("Cannot enable collaborative mode without a registry resource id");
        return;
      }
      const authority = resourceAuthorityForPersistMode(
        mode,
        registryResourceId ?? "",
      );
      try {
        await setResourceAuthority(context.workspaceRoot, canvasPath, authority);
        setPersistMode(mode);
        callbacks.onPersistModeChange?.(mode);
      } catch (err) {
        console.error("Failed to persist canvas mode", err);
      }
    },
    [callbacks, canvasPath, context.workspaceRoot, registryResourceId],
  );

  useEffect(() => {
    const fromCatalog = resolveRegistryResourceId(context.catalog, canvasPath);
    if (fromCatalog) {
      setRegistryResourceId(fromCatalog);
    }
    if (!context.workspaceRoot) {
      if (!fromCatalog) {
        setRegistryResourceId(undefined);
      }
      return;
    }
    let cancelled = false;
    void getResourceStat(context.workspaceRoot, canvasPath).then((stat) => {
      if (cancelled) return;
      const statId = looksLikeLatticeResourceId(stat.resource_id)
        ? stat.resource_id
        : undefined;
      if (!fromCatalog) {
        setRegistryResourceId(statId);
      }
      const registryId = fromCatalog ?? statId;
      if (registryId) {
        setPersistMode(persistModeFromResourceStat(stat, registryId));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [canvasPath, context.catalog, context.workspaceRoot]);

  const collaborativeAvailable =
    registryResourceId !== undefined && looksLikeLatticeResourceId(registryResourceId);
  const remoteProviderEnabled = collaborativeAvailable && cloudSession?.signedIn === true;

  useEffect(() => {
    if (!collaborativeAvailable && persistMode === "collaborative") {
      setPersistMode("plain");
    }
  }, [collaborativeAvailable, persistMode]);

  const workspaceRootRef = useRef(context.workspaceRoot);
  workspaceRootRef.current = context.workspaceRoot;
  const canvasPathRef = useRef(canvasPath);
  canvasPathRef.current = canvasPath;
  const onRevisionChangeRef = useRef(callbacks.onRevisionChange);
  onRevisionChangeRef.current = callbacks.onRevisionChange;

  const materializeControllerRef = useRef(
    createSerializedSaveController<string | null>({
      initialRevision: revision,
      save: async (baseRevision) => {
        const ydoc = collabHandleRef.current?.ydoc;
        const root = workspaceRootRef.current;
        if (!ydoc || !root) return baseRevision;
        return materializeCollabCanvas(
          {
            ydoc,
            io: createNativeCanvasFileIO(root, canvasPathRef.current),
          },
          baseRevision,
        );
      },
      onRevision: (next) => {
        if (next) onRevisionChangeRef.current(next);
      },
      isConflict: isCanvasMaterializeConflict,
      onStatus: () => undefined,
      savedIndicatorMs: 0,
    }),
  );
  const materializeController = materializeControllerRef.current;

  const jsonRef = useRef(json);
  jsonRef.current = json;

  const markCollabDirty = useCallback(() => {
    if (shouldScheduleCollabCheckpoint(persistMode)) {
      materializeController.markDirty(settings.editor.autosaveDelayMs);
    }
  }, [materializeController, persistMode, settings.editor.autosaveDelayMs]);

  useEffect(() => {
    if (persistMode !== "collaborative") {
      setCollabLoading(false);
      setCollabError(null);
      return;
    }
    if (!context.workspaceRoot || !registryResourceId) {
      setCollabError("Collaborative mode requires a registry resource id.");
      return;
    }

    let cancelled = false;
    setCollabLoading(true);
    setCollabError(null);
    void openCollabSession({
      workspaceRoot: context.workspaceRoot,
      docId: registryResourceId,
      pagePath: canvasPath,
      remoteProviderEnabled,
      onError: (message) => {
        if (!cancelled) setCollabError(message);
      },
    })
      .then((handle) => {
        if (cancelled) {
          handle.dispose();
          return;
        }
        try {
          if (handle.created || canvasYDocIsEmpty(handle.ydoc)) {
            applyCanvasDataToYDoc(handle.ydoc, parseCanvas(jsonRef.current));
          }
        } catch (error) {
          handle.dispose();
          throw error;
        }
        collabHandleRef.current = handle;
        setCollabYdoc(handle.ydoc);
        setCollabLoading(false);
      })
      .catch((error) => {
        if (!cancelled) {
          setCollabError(String(error));
          setCollabLoading(false);
        }
      });

    return () => {
      cancelled = true;
      void materializeController.flush().finally(() => {
        collabHandleRef.current?.dispose();
        collabHandleRef.current = null;
        setCollabYdoc(null);
      });
    };
  }, [
    canvasPath,
    context.workspaceRoot,
    materializeController,
    persistMode,
    registryResourceId,
    remoteProviderEnabled,
  ]);

  useEffect(() => {
    if (!collabYdoc) return;
    const onUpdate = (_update: Uint8Array, origin: unknown) => {
      if (origin === CANVAS_SEED_ORIGIN) return;
      markCollabDirty();
    };
    collabYdoc.on("update", onUpdate);
    return () => {
      collabYdoc.off("update", onUpdate);
    };
  }, [collabYdoc, markCollabDirty]);

  useEffect(
    () => () => {
      void materializeController.flush();
      materializeController.dispose();
    },
    [materializeController],
  );

  const collabAdapter = useMemo(() => {
    if (!collabYdoc) return undefined;
    return createCollabCanvasAdapter({
      ydoc: collabYdoc,
      canvasPath,
      getRevision: () => revisionRef.current,
      onLocalChange: markCollabDirty,
    });
  }, [canvasPath, collabYdoc, markCollabDirty]);

  const adapter = shouldPatchPlainCanvas(persistMode) ? nativeAdapter : collabAdapter;
  const viewerKey = `${canvasPath}#${context.reloadToken}#${persistMode}#${registryResourceId ?? "path"}`;

  return (
    <CanvasViewer
      key={viewerKey}
      json={json}
      canvasPath={canvasPath}
      workspaceRoot={context.workspaceRoot ?? undefined}
      resources={context.resources}
      adapter={adapter}
      baseRevision={revision}
      onRevisionChange={callbacks.onRevisionChange}
      onOpenFile={callbacks.onOpenFile}
      persistMode={persistMode}
      collaborativeAvailable={collaborativeAvailable}
      onPersistModeChange={handlePersistModeChange}
      collabYdoc={collabYdoc}
      collabLoading={collabLoading}
      collabError={collabError}
    />
  );
}
