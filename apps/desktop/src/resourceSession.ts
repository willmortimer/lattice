import type { DataAppSnapshot } from "./data/types";
import type { InterfaceSummary } from "./data/interfaces";
import type { PageIO } from "./editor/pageIO";
import type { ArtifactManifestDto } from "./lib/artifactRun";
import type { ResourceEncoding, ResourceInspection } from "./lib/resourceRuntime";
import type { TaskManifestDto } from "./lib/taskRun";
import type { WorkflowManifestDto } from "./lib/workflowRun";
import type { Resource } from "./types";

/** The resource currently open in the main surface.
 *
 * Keeping the payload and its kind together prevents the shell from rendering
 * a stale page, canvas, or data view after a new resource starts loading.
 */
export type OpenResourceSession =
  | {
      kind: "page";
      resource: Resource;
      content: string;
      revision: string | null;
      io: PageIO;
    }
  | {
      kind: "canvas";
      resource: Resource;
      json: unknown;
      revision: string;
    }
  | {
      kind: "data-app";
      resource: Resource;
      snapshot: DataAppSnapshot;
    }
  | {
      kind: "interface";
      resource: Resource;
      snapshot: DataAppSnapshot;
      interfaceDef: InterfaceSummary;
    }
  | {
      kind: "dataset";
      resource: Resource;
    }
  | {
      kind: "text";
      resource: Resource;
      inspection: ResourceInspection;
      content: string;
      revision: string;
      offset: number;
      totalSize: number;
      truncated: boolean;
      encoding: ResourceEncoding;
      editable: boolean;
    }
  | {
      kind: "notebook";
      resource: Resource;
      content: string;
      revision: string;
    }
  | {
      kind: "task";
      resource: Resource;
      manifest: TaskManifestDto;
    }
  | {
      kind: "workflow";
      resource: Resource;
      manifest: WorkflowManifestDto;
    }
  | {
      kind: "artifact";
      resource: Resource;
      manifest: ArtifactManifestDto;
    }
  | {
      kind: "unknown";
      resource: Resource;
    };

export function resourceForSession(session: OpenResourceSession | null): Resource | null {
  return session?.resource ?? null;
}
