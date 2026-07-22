#!/usr/bin/env bash
# Seed First Look analytical datasets and regenerate embedded template catalogs.
# Run from repo root: nxr prepare-first-look  (or: bash scripts/prepare-first-look.sh)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

demo="templates/workspaces/demo/files"

echo "prepare-first-look: seed_demo_events"
cargo run -p lattice-datasets --example seed_demo_events --quiet

echo "prepare-first-look: seed_demo_places"
cargo run -p lattice-datasets --example seed_demo_places --quiet

echo "prepare-first-look: compile-templates"
pnpm compile-templates

required=(
  "$demo/Home.md"
  "$demo/Automations/Contact intake.workflow.yaml"
  "$demo/Dashboards/Signups by region.vl.json"
  "$demo/Data/Events.dataset/facts/year=2026/month=07/signups.parquet"
  "$demo/Data/Events.dataset/annotations.sqlite"
  "$demo/Data/Places.dataset/facts/places.parquet"
  "apps/desktop/src/demoWorkspace.generated.ts"
  "crates/lattice-core/src/template_catalog.generated.rs"
)

missing=()
for path in "${required[@]}"; do
  if [[ ! -e "$path" ]]; then
    missing+=("$path")
  fi
done

if ((${#missing[@]} > 0)); then
  echo "prepare-first-look: missing expected artifacts:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  exit 1
fi

echo "prepare-first-look: ok (${#required[@]} paths verified)"
