#!/usr/bin/env bash
# Controller (Mac) → nixdev (WSL) Lattice Windows winbuild leaf.
# Rsyncs the lattice checkout, syncs onto DevDrive, runs winbuild tasks.
#
# Env:
#   NIXDEV_HOST          default will@nixdev
#   LATTICE_REMOTE       default /home/will/Developer/lattice-ecosystem/lattice
#   WINBUILD_DEST        default /mnt/d/lattice
#   WINBUILD_TASKS       default "probe ensure-toolchain cargo-check-core"
#                        (space-separated winbuild task names)
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

host="${NIXDEV_HOST:-will@nixdev}"
remote="${LATTICE_REMOTE:-/home/will/Developer/lattice-ecosystem/lattice}"
dest="${WINBUILD_DEST:-/mnt/d/lattice}"
tasks="${WINBUILD_TASKS:-probe ensure-toolchain cargo-check-core}"
manifest='D:\lattice\.winbuild.json'

# Prefer NixPlane sync helper when available on the remote host.
nixplane_sync="${NIXPLANE_REMOTE:-/home/will/Developer/NixPlane}/scripts/winbuild/sync.sh"

echo "lattice-winbuild-remote: rsync → ${host}:${remote}" >&2
rsync -az --delete \
  --exclude target \
  --exclude result \
  --exclude .direnv \
  --exclude node_modules \
  --exclude apps/desktop/dist \
  --exclude .git/modules \
  "${root}/" "${host}:${remote}/"

# lattice Cargo.toml path-deps ../kernelfs — mirror sibling on nixdev + DevDrive.
kernelfs_root="$(cd "${root}/../kernelfs" 2>/dev/null && pwd || true)"
kernelfs_remote="${KERNELFS_REMOTE:-/home/will/Developer/lattice-ecosystem/kernelfs}"
if [[ -n "$kernelfs_root" && -d "$kernelfs_root" ]]; then
  echo "lattice-winbuild-remote: rsync kernelfs → ${host}:${kernelfs_remote}" >&2
  rsync -az --delete \
    --exclude target \
    --exclude result \
    --exclude .direnv \
    --exclude .git/modules \
    "${kernelfs_root}/" "${host}:${kernelfs_remote}/"
fi

echo "lattice-winbuild-remote: sync + winbuild (${tasks}) on ${host}" >&2
# shellcheck disable=SC2086
ssh -o BatchMode=yes "$host" bash -s <<EOF
set -euo pipefail
export PATH="\$HOME/bin:/mnt/d/NixPlane/bin:\$PATH"
cd $(printf '%q' "$remote")

if [[ -x $(printf '%q' "$nixplane_sync") ]]; then
  $(printf '%q' "$nixplane_sync") --dest $(printf '%q' "$dest") .
else
  mkdir -p $(printf '%q' "$dest")
  rsync -a --delete \
    --exclude target --exclude result --exclude .direnv --exclude node_modules \
    ./ $(printf '%q' "$dest")/
fi

# Sibling path-dep: D:\lattice\..\kernelfs → D:\kernelfs
if [[ -d $(printf '%q' "$kernelfs_remote") ]]; then
  mkdir -p /mnt/d/kernelfs
  rsync -a --delete \
    --exclude target --exclude result --exclude .direnv \
    $(printf '%q' "$kernelfs_remote")/ /mnt/d/kernelfs/
fi

command -v winbuild.exe >/dev/null || command -v winbuild >/dev/null
WB="\$(command -v winbuild.exe || command -v winbuild)"
cd $(printf '%q' "$dest")

# Controller expands WINBUILD_TASKS into the remote for-loop word list.
for task in ${tasks}; do
  echo "lattice-winbuild-remote: run \$task" >&2
  # Windows .exe under WSL can drain SSH stdin; never let it see the heredoc.
  # Scope doctor to the task being run — global doctor fails when optional
  # packaging tools (pnpm) are missing even for cargo-only tasks.
  "\$WB" doctor "\$task" --file $(printf '%q' "$manifest") </dev/null
  "\$WB" run "\$task" --file $(printf '%q' "$manifest") </dev/null
done

echo "lattice-winbuild-remote: OK"
EOF
