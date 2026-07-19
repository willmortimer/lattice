{
  description = "Lattice — local-first open-native workspace (dev shell and tasks)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nxr.url = "github:willmortimer/nxr";
    flake-parts.follows = "nxr/flake-parts";
  };

  outputs =
    inputs@{
      self,
      flake-parts,
      nxr,
      nixpkgs,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ nxr.flakeModules.default ];

      systems = [
        "aarch64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        {
          pkgs,
          system,
          lib,
          ...
        }:
        let
          toolchain =
            with pkgs;
            [
              rustc
              cargo
              rustfmt
              clippy
              rust-analyzer
              nodejs_22
              pnpm
              pkg-config
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [ libiconv ]
            ++ lib.optionals pkgs.stdenv.isLinux [
              # Tauri 2 Linux prerequisites
              gtk3
              webkitgtk_4_1
              libayatana-appindicator
              librsvg
              openssl
            ];

          descriptions = {
            test = "Run cargo test --workspace";
            lint = "Run Clippy (-D warnings) and rustfmt --check";
            fmt = "Format all Rust sources";
            check = "CI gate: fmt, clippy, tests, desktop + site builds";
            site-dev = "Start the Astro marketing/docs site";
            site-build = "Build the static marketing/docs site";
            docs-sync = "Regenerate site docs content from docs/";
            compile-theme = "Compile themes/*.theme.yaml into CSS/TS tokens";
            compile-templates = "Validate templates and regenerate embedded catalogs";
            desktop-dev = "Native Tauri window + Vite HMR (re-seeds First Look in target/dev-home)";
            desktop-web = "Browser-only React demo UI (no Tauri / filesystem)";
            desktop-perf = "Playwright browser perf harness against the Vite demo";
            desktop-perf-tauri = "Native WebView perf via tauri-plugin-playwright";
            desktop = "Native Tauri window without Vite (reuses apps/desktop/dist)";
            desktop-build = "Release binary, unbundled (tauri build --no-bundle)";
            desktop-ui-build = "Build the desktop Vite frontend only";
            desktop-install = "macOS: signed .app → /Applications (Apple Development)";
            ok = "No-op success (nxr task DAG join)";
          };

          scripts = {
            test = ''
              exec cargo test --workspace "$@"
            '';
            lint = ''
              cargo clippy --workspace --all-targets -- -D warnings
              cargo fmt --all --check
            '';
            fmt = ''
              exec cargo fmt --all "$@"
            '';
            check = ''
              cargo fmt --all --check
              cargo clippy --workspace --all-targets -- -D warnings
              cargo test --workspace
              pnpm install --frozen-lockfile
              pnpm --filter @lattice/desktop build
              pnpm --filter @lattice/site build
            '';
            site-dev = ''
              pnpm install
              exec pnpm --filter @lattice/site dev "$@"
            '';
            site-build = ''
              pnpm install
              exec pnpm --filter @lattice/site build "$@"
            '';
            docs-sync = ''
              exec pnpm --filter @lattice/site sync-docs "$@"
            '';
            compile-theme = ''
              exec pnpm --filter @lattice/desktop compile-theme "$@"
            '';
            compile-templates = ''
              exec pnpm compile-templates "$@"
            '';
            desktop-dev = ''
              pnpm install
              exec pnpm --filter @lattice/desktop tauri:dev "$@"
            '';
            desktop-web = ''
              pnpm install
              exec pnpm --filter @lattice/desktop dev "$@"
            '';
            desktop-perf = ''
              pnpm install
              pnpm --filter @lattice/desktop exec playwright install chromium
              exec pnpm --filter @lattice/desktop test:perf "$@"
            '';
            desktop-perf-tauri = ''
              pnpm install
              exec pnpm --filter @lattice/desktop test:perf:tauri "$@"
            '';
            desktop = ''
              pnpm install
              if [ ! -f apps/desktop/dist/index.html ]; then
                echo "lattice-desktop: building frontend into apps/desktop/dist…"
                pnpm --filter @lattice/desktop build
              else
                echo "lattice-desktop: reusing apps/desktop/dist (rebuild with: pnpm --filter @lattice/desktop build)"
              fi
              exec pnpm --filter @lattice/desktop exec tauri dev --config '{"build":{"beforeDevCommand":""}}' "$@"
            '';
            desktop-build = ''
              pnpm install
              exec pnpm --filter @lattice/desktop tauri build --no-bundle "$@"
            '';
            desktop-ui-build = ''
              pnpm install --frozen-lockfile
              exec pnpm --filter @lattice/desktop build "$@"
            '';
            desktop-install = ''
              if [ "$(uname -s)" != "Darwin" ]; then
                echo "desktop-install: macOS only" >&2
                exit 1
              fi

              : "''${APPLE_SIGNING_IDENTITY:?Set APPLE_SIGNING_IDENTITY (see .env.example / docs/dev/environment.md)}"

              if [ -z "''${APPLE_TEAM_ID:-}" ]; then
                echo "desktop-install: warning: APPLE_TEAM_ID unset (ok for local Apple Development; needed later for notarization)" >&2
              fi

              # Prefer real Xcode when the Nix shell points xcode-select at the SDK stub.
              if [ -d /Applications/Xcode.app/Contents/Developer ]; then
                export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
              elif [ -d /Library/Developer/CommandLineTools ]; then
                export DEVELOPER_DIR=/Library/Developer/CommandLineTools
              fi

              pnpm install
              pnpm --filter @lattice/desktop exec tauri build --bundles app

              # Cargo workspace target dir is repo-root `target/`, not src-tauri/target.
              app_src="target/release/bundle/macos/Lattice.app"
              if [ ! -d "$app_src" ]; then
                # Older / alternate layouts may still use the crate-local target.
                alt_src="apps/desktop/src-tauri/target/release/bundle/macos/Lattice.app"
                if [ -d "$alt_src" ]; then
                  app_src="$alt_src"
                else
                  echo "desktop-install: missing bundle at $app_src (also checked $alt_src)" >&2
                  exit 1
                fi
              fi

              # Ensure the identity we expect is on the bundle (Tauri may already have signed).
              if ! codesign --force --deep --sign "$APPLE_SIGNING_IDENTITY" "$app_src"; then
                echo "desktop-install: codesign failed for identity: $APPLE_SIGNING_IDENTITY" >&2
                exit 1
              fi

              dest="''${LATTICE_INSTALL_DIR:-/Applications}/Lattice.app"
              echo "desktop-install: installing → $dest"
              rm -rf "$dest"
              ditto "$app_src" "$dest"
              codesign -dv --verbose=2 "$dest" || true
              echo "desktop-install: done. Open with: open \"$dest\""
            '';
            ok = ''
              true
            '';
          };

          latticeScripts = lib.mapAttrs (
            name: script:
            pkgs.writeShellApplication {
              name = "lattice-${name}";
              runtimeInputs = toolchain;
              text = script;
            }
          ) scripts;
        in
        {
          packages.nxr = nxr.packages.${system}.nxr;

          nxr.shellIntegration = {
            enable = true;
            devShells = [ "default" ];
          };

          nxr.apps = lib.mapAttrs (name: script: {
            description = descriptions.${name};
            runtimeInputs = toolchain;
            inherit script;
          }) scripts;

          # Orchestration around flake apps. Leaf apps stay authoritative;
          # `nxr task` / `nxr graph` use this metadata.
          nxr.tasks = {
            test = {
              description = "Run cargo tests";
              app = "test";
              category = "validation";
            };
            lint = {
              description = "Clippy + rustfmt check";
              app = "lint";
              category = "validation";
            };
            fmt = {
              description = "Format Rust sources";
              app = "fmt";
              category = "dev";
            };
            check = {
              description = "Monolithic CI gate";
              app = "check";
              category = "validation";
              aliases = [ "ci" ];
            };
            validate = {
              description = "Parallel validation (lint ∥ test ∥ desktop UI ∥ site)";
              dependsOn = [
                "lint"
                "test"
                "desktop-ui-build"
                "site-build"
              ];
              app = "ok";
              category = "validation";
            };
            compile-theme = {
              description = "Compile theme tokens";
              app = "compile-theme";
              category = "codegen";
            };
            compile-templates = {
              description = "Compile workspace templates";
              app = "compile-templates";
              category = "codegen";
            };
            codegen = {
              description = "Compile theme tokens and workspace templates";
              dependsOn = [
                "compile-theme"
                "compile-templates"
              ];
              app = "ok";
              category = "codegen";
              aliases = [ "compile" ];
            };
            docs-sync = {
              description = "Sync docs/ into the site";
              app = "docs-sync";
              category = "site";
            };
            site-dev = {
              description = "Astro marketing/docs site (dev)";
              app = "site-dev";
              category = "site";
            };
            site-build = {
              description = "Build marketing/docs site";
              app = "site-build";
              category = "site";
            };
            desktop-dev = {
              description = "Tauri + Vite HMR";
              app = "desktop-dev";
              category = "desktop";
            };
            desktop-web = {
              description = "Browser-only demo UI";
              app = "desktop-web";
              category = "desktop";
            };
            desktop = {
              description = "Native without Vite";
              app = "desktop";
              category = "desktop";
            };
            desktop-build = {
              description = "Unbundled release binary";
              app = "desktop-build";
              category = "desktop";
            };
            desktop-ui-build = {
              description = "Build desktop frontend (Vite)";
              app = "desktop-ui-build";
              category = "validation";
            };
            desktop-install = {
              description = "Sign and install Lattice.app locally (macOS)";
              app = "desktop-install";
              category = "desktop";
              aliases = [ "install" ];
            };
            desktop-perf = {
              description = "Browser perf harness";
              app = "desktop-perf";
              category = "desktop";
            };
            desktop-perf-tauri = {
              description = "Tauri WebView perf harness";
              app = "desktop-perf-tauri";
              category = "desktop";
            };
          };

          devShells.default = pkgs.mkShell {
            packages = toolchain ++ lib.attrValues latticeScripts;
            shellHook = ''
              echo "lattice dev shell — rust $(rustc --version | cut -d' ' -f2), node $(node --version), pnpm $(pnpm --version)"
              echo "runner: nxr list | nxr <app> | nxr task <name> [-j N] | nxr graph <name>"
              echo "legacy: lattice-{test,lint,fmt,check,site-*,compile-*,desktop*} (also: nix run .#<app>)"
            '';
          };
        };
    };
}
