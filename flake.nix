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
            site-deploy = "Build the site and deploy to Cloudflare Pages (lattice-dop)";
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
            desktop-install = "macOS: signed .app with voice → /Applications (Apple Development)";
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
            site-deploy = ''
              pnpm install
              pnpm --filter @lattice/site build
              # Cloudflare Pages project is "lattice"; public host is lattice-dop.pages.dev.
              # Prefer CLOUDFLARE_API_TOKEN from sops (see secrets/README.md). Falls back
              # to interactive `wrangler login` credentials in ~/.wrangler if unset.
              if [ -z "''${CLOUDFLARE_API_TOKEN:-}" ]; then
                echo "site-deploy: CLOUDFLARE_API_TOKEN unset — using wrangler OAuth store if present." >&2
                echo "  sops path: sops exec-env secrets/cloudflare.env -- nix run .#site-deploy" >&2
              fi
              exec wrangler pages deploy site/dist \
                --project-name=lattice \
                --commit-dirty=true \
                "$@"
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
              # Match desktop-dev on macOS so release binaries include voice capture.
              # Linux CI stays featureless (no Swift FluidAudio bridges).
              if [ "$(uname -s)" = "Darwin" ]; then
                exec pnpm --filter @lattice/desktop exec tauri build --no-bundle --features voice-embedded "$@"
              else
                exec pnpm --filter @lattice/desktop tauri build --no-bundle "$@"
              fi
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

              pnpm install
              # Keep the Nix apple-sdk DEVELOPER_DIR/SDKROOT for the Cargo build.
              # Overriding to Xcode.app here mixes Xcode's MacOSX.sdk headers with
              # Nix libcxx and breaks libduckdb-sys (uint8_t / intmax_t / _CTYPE_*).
              # Same voice path as `nxr desktop-dev` / `pnpm tauri:dev` — without this,
              # Settings → Voice reports Unavailable (Cargo default features are empty).
              pnpm --filter @lattice/desktop exec tauri build --bundles app --features voice-embedded

              # Thin-client sidecars (semantic + voice) must sit beside lattice-desktop.
              echo "desktop-install: building latticed / lattice-embed-host / lattice-voice-host"
              cargo build --release -p lattice-daemon --bin latticed
              cargo build --release -p lattice-embed-host --bin lattice-embed-host --features llama-cpp
              cargo build --release -p lattice-voice-host --bin lattice-voice-host --features fluidaudio || \
                cargo build --release -p lattice-voice-host --bin lattice-voice-host

              echo "desktop-install: verifying production sidecars"
              for bin in latticed lattice-embed-host lattice-voice-host; do
                if [ ! -f "target/release/$bin" ]; then
                  echo "desktop-install: missing target/release/$bin after build" >&2
                  exit 1
                fi
              done
              backends="$(target/release/lattice-embed-host backends || true)"
              echo "desktop-install: lattice-embed-host backends:"$'\n'"$backends"
              if ! printf '%s\n' "$backends" | grep -qx 'llama-cpp'; then
                echo "desktop-install: lattice-embed-host must list llama-cpp (build with --features llama-cpp)" >&2
                exit 1
              fi

              # Prefer real Xcode for codesign when the Nix shell points xcode-select
              # at the SDK stub (codesign itself does not need the Nix C++ toolchain).
              if [ -d /Applications/Xcode.app/Contents/Developer ]; then
                export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
              elif [ -d /Library/Developer/CommandLineTools ]; then
                export DEVELOPER_DIR=/Library/Developer/CommandLineTools
              fi

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

              # Swift bridges use @loader_path; copy dylibs next to the Mach-O in the bundle.
              macos_dir="$app_src/Contents/MacOS"
              for dylib in libLatticeVoiceBridge.dylib libLatticeAudioBridge.dylib; do
                src="target/release/$dylib"
                if [ -f "$src" ]; then
                  cp -f "$src" "$macos_dir/$dylib"
                  echo "desktop-install: bundled $dylib"
                else
                  echo "desktop-install: warning: missing $src (voice may fail at runtime)" >&2
                fi
              done

              # Semantic search + voice thin-clients expect latticed (and embed-host)
              # as MacOS siblings of the app binary (see docs/search/…).
              for bin in latticed lattice-embed-host lattice-voice-host; do
                src="target/release/$bin"
                if [ ! -f "$src" ]; then
                  echo "desktop-install: missing $src (required production sidecar)" >&2
                  exit 1
                fi
                cp -f "$src" "$macos_dir/$bin"
                chmod +x "$macos_dir/$bin"
                echo "desktop-install: bundled $bin"
              done

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

          # Site scripts only need Node. Wrangler comes from a thin npx wrapper —
          # nixpkgs#wrangler builds the workers-sdk monorepo (~GiB) and currently
          # fails on Darwin (EBADF during tsup). Published npm CLI is enough for
          # Pages deploy + `wrangler login`.
          siteNodeToolchain = with pkgs; [
            nodejs_22
            pnpm
          ];

          # sops + age for decrypting secrets/ when deploying from the ops shell.
          secretsToolchain = with pkgs; [
            sops
            age
          ];

          wrangler = pkgs.writeShellApplication {
            name = "wrangler";
            runtimeInputs = [ pkgs.nodejs_22 ];
            text = ''
              exec npx --yes wrangler@4 "$@"
            '';
          };

          siteToolchain = siteNodeToolchain ++ [ wrangler ] ++ secretsToolchain;

          siteScriptNames = [
            "site-dev"
            "site-build"
            "site-deploy"
            "docs-sync"
          ];

          runtimeInputsFor =
            name:
            if name == "site-deploy" then
              siteToolchain
            else if builtins.elem name siteScriptNames then
              siteNodeToolchain
            else
              toolchain;

          latticeScripts = lib.mapAttrs (
            name: script:
            pkgs.writeShellApplication {
              name = "lattice-${name}";
              runtimeInputs = runtimeInputsFor name;
              text = script;
            }
          ) scripts;

          # Keep wrangler out of the default direnv shell; only ops + site-deploy pull it.
          defaultLatticeScripts = builtins.removeAttrs latticeScripts [ "site-deploy" ];
        in
        {
          packages.nxr = nxr.packages.${system}.nxr;
          packages.wrangler = wrangler;

          nxr.shellIntegration = {
            enable = true;
            # `default` = day-to-day Rust/desktop; `ops` = site publish / Cloudflare.
            devShells = [
              "default"
              "ops"
            ];
          };

          nxr.apps = lib.mapAttrs (name: script: {
            description = descriptions.${name};
            runtimeInputs = runtimeInputsFor name;
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
            site-deploy = {
              description = "Deploy marketing/docs site to Cloudflare Pages";
              app = "site-deploy";
              category = "site";
              aliases = [ "deploy-site" ];
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

          # Day-to-day app development (Rust, desktop, site local preview).
          # direnv `use flake` loads this shell.
          # site-deploy / wrangler stay in .#ops so default reload stays light.
          devShells.default = pkgs.mkShell {
            packages = toolchain ++ lib.attrValues defaultLatticeScripts;
            shellHook = ''
              echo "lattice dev shell — rust $(rustc --version | cut -d' ' -f2), node $(node --version), pnpm $(pnpm --version)"
              echo "runner: nxr list | nxr <app> | nxr task <name> [-j N] | nxr graph <name>"
              echo "legacy: lattice-{test,lint,fmt,check,site-*,compile-*,desktop*} (also: nix run .#<app>)"
              echo "ops / Cloudflare: nix develop .#ops"
            '';
          };

          # Lightweight publish shell: node/pnpm + wrangler (npx wrapper) for
          # site build/deploy. Activate with `nix develop .#ops` (does not
          # replace direnv default).
          # Auth: `wrangler login` stores OAuth tokens under your home directory
          # (~/Library/Preferences/.wrangler on macOS), not in the Nix store —
          # so login survives shell exits. CI can use CLOUDFLARE_API_TOKEN instead.
          # Prefer this shell for interactive wrangler; `nix run .#site-deploy`
          # works the same once authenticated. First `wrangler` call needs npm
          # network access to fetch the CLI into the npx cache.
          devShells.ops = pkgs.mkShell {
            packages = siteToolchain ++ [
              latticeScripts.site-build
              latticeScripts.site-deploy
              latticeScripts.docs-sync
            ];
            shellHook = ''
              echo "lattice ops shell — wrangler (npx wrangler@4), node $(node --version), pnpm $(pnpm --version)"
              echo "secrets: sops/age available — see secrets/README.md"
              echo "auth: sops secrets/cloudflare.env   # or: wrangler login"
              echo "deploy: nix run .#site-deploy"
              echo "        sops exec-env secrets/cloudflare.env -- nix run .#site-deploy"
              echo "live: https://lattice-dop.pages.dev/"
            '';
          };
        };
    };
}
