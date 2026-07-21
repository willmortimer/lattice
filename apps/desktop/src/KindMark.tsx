import type { JSX } from "react";
import type { ResourceKind } from "./types";

/**
 * Kind marks: every resource kind drawn as a small constellation on the
 * same 3x3 lattice grid (points at 4/10/16 in a 20x20 viewBox). One
 * drawing system — nodes and thin edges — so the sidebar, placeholders,
 * and empty state all speak the product's grid language.
 */
const MARKS: Record<ResourceKind, JSX.Element> = {
  // Text block: three rules, the last one trailing off.
  page: (
    <>
      <path d="M4 5.5h12M4 10h12M4 14.5h7" />
    </>
  ),
  // Spatial frame: four corner nodes joined into a plane.
  canvas: (
    <>
      <path d="M4 4h12v12H4z" />
      <circle cx="4" cy="4" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="16" cy="4" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="4" cy="16" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="16" cy="16" r="1.7" fill="currentColor" stroke="none" />
    </>
  ),
  // Records: the full grid of typed rows and fields.
  "data-app": (
    <>
      {[4, 10, 16].flatMap((y) =>
        [4, 10, 16].map((x) => (
          <circle key={`${x}-${y}`} cx={x} cy={y} r="1.5" fill="currentColor" stroke="none" />
        )),
      )}
    </>
  ),
  // Analytical columns rising off the baseline.
  dataset: (
    <>
      <path d="M5 16V11M10 16V6.5M15 16V9" strokeWidth="2.4" />
    </>
  ),
  // Notebook cell: input over output.
  notebook: (
    <>
      <rect x="4" y="4" width="12" height="12" rx="1.5" />
      <path d="M4 10h12" />
      <circle cx="7" cy="7" r="1.3" fill="currentColor" stroke="none" />
    </>
  ),
  // A drawn stroke.
  ink: (
    <>
      <path d="M4 14c3-6 5 4 8-2 1.4-2.8 2.6-3.4 4-3.5" />
    </>
  ),
  // Three nodes composed into one component.
  artifact: (
    <>
      <path d="M10 4.5 15.5 14h-11z" />
      <circle cx="10" cy="4.5" r="1.5" fill="currentColor" stroke="none" />
      <circle cx="15.5" cy="14" r="1.5" fill="currentColor" stroke="none" />
      <circle cx="4.5" cy="14" r="1.5" fill="currentColor" stroke="none" />
    </>
  ),
  // An application: a frame with a live node inside.
  app: (
    <>
      <rect x="4" y="4" width="12" height="12" rx="2.5" />
      <circle cx="10" cy="10" r="2" fill="currentColor" stroke="none" />
    </>
  ),
  // Steps joined into a path.
  workflow: (
    <>
      <path d="M4.5 15.5 10 10l5.5-5.5" />
      <circle cx="4.5" cy="15.5" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="10" cy="10" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="15.5" cy="4.5" r="1.7" fill="currentColor" stroke="none" />
    </>
  ),
  // A runnable unit: node plus its completion tick.
  task: (
    <>
      <circle cx="10" cy="10" r="6" />
      <path d="m7.5 10 1.8 1.8L13 8.2" />
    </>
  ),
  // Inputs feeding a generated output node.
  derived: (
    <>
      <circle cx="4.5" cy="6" r="1.5" fill="currentColor" stroke="none" />
      <circle cx="4.5" cy="14" r="1.5" fill="currentColor" stroke="none" />
      <path d="M6.2 6.8 13 10l-6.8 3.2" />
      <circle cx="15" cy="10" r="2.2" />
    </>
  ),
  // Folder: an open frame on the grid.
  folder: (
    <>
      <path d="M3.5 7.5h4l1.5-2h7.5v10.5h-13z" />
    </>
  ),
  // Any ordinary file: one honest node on the grid.
  file: (
    <>
      <circle cx="10" cy="10" r="2" fill="currentColor" stroke="none" />
      <path d="M10 3.5v3M10 13.5v3M3.5 10h3M13.5 10h3" opacity="0.55" />
    </>
  ),
};

export function KindMark({ kind, size = 15 }: { kind: ResourceKind; size?: number }) {
  return (
    <svg
      className="kind-mark"
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {MARKS[kind] ?? MARKS.file}
    </svg>
  );
}

export const KIND_LABELS: Record<ResourceKind, string> = {
  page: "Page",
  canvas: "Canvas",
  "data-app": "Data app",
  dataset: "Dataset",
  notebook: "Notebook",
  ink: "Ink",
  artifact: "Artifact",
  app: "App",
  workflow: "Workflow",
  task: "Task",
  derived: "Derived",
  folder: "Folder",
  file: "File",
};
