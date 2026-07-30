{
  description = "Lattice — local-first open-native workspace (dev shell and tasks)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # Pin releases — do not track nxr main from consumer flakes.
    nxr.url = "github:willmortimer/nxr/v3.4.0";
    flake-parts.follows = "nxr/flake-parts";
    flake-schemas.url = "github:DeterminateSystems/flake-schemas";
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
              sccache
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
            rust-test = "Run cargo test --workspace";
            rust-fmt-check = "cargo fmt --all --check";
            rust-clippy = "cargo clippy --workspace --all-targets -D warnings";
            lint = "Clippy + rustfmt check (compat; prefer rust-clippy ∥ rust-fmt-check)";
            fmt = "Format all Rust sources";
            check = "Monolithic escape hatch: fmt, clippy, tests, desktop build";
            js-deps = "pnpm install --frozen-lockfile --prefer-offline";
            desktop-ui-test = "Vitest for the desktop frontend";
            generated-theme-check = "Compile themes and fail on git drift";
            generated-template-check = "Compile templates and fail on git drift";
            flake-check = "nix flake check";
            compile-theme = "Compile themes/*.theme.yaml into CSS/TS tokens";
            compile-templates = "Validate templates and regenerate embedded catalogs";
            prepare-first-look = "Seed First Look demo datasets and regenerate template catalogs";
            desktop-dev = "Native Tauri window + Vite HMR (re-seeds First Look in target/dev-home)";
            desktop-web = "Browser-only React demo UI (no Tauri / filesystem)";
            desktop-perf = "Playwright browser perf harness against the Vite demo";
            desktop-perf-tauri = "Native WebView perf via tauri-plugin-playwright";
            desktop = "Native Tauri window without Vite (reuses apps/desktop/dist)";
            desktop-build = "Release binary, unbundled (tauri build --no-bundle)";
            desktop-ui-build = "Build the desktop Vite frontend only";
            desktop-install = "macOS: signed .app with voice → /Applications (Apple Development)";
            desktop-release = "macOS release DAG join (env → build → sign → notary → dmg)";
            desktop-release-internal = "Internal channel app (bundle id dev.lattice.desktop.dev)";
            release-env-validate = "Validate Apple Developer ID + notarytool env";
            desktop-tauri-bundle = "Tauri app bundle with voice-embedded";
            build-latticed = "Release-build latticed sidecar";
            build-agentd = "Release-build lattice-agentd sidecar";
            build-embed-host = "Release-build lattice-embed-host (llama-cpp)";
            build-voice-host = "Release-build lattice-voice-host";
            verify-sidecars = "Verify release sidecar binaries + embed backends";
            assemble-app = "Copy sidecars/dylibs into Lattice.app";
            codesign-app = "Developer ID codesign (hardened runtime)";
            notarize-app = "Submit Lattice.app to Apple notarytool";
            staple-app = "Staple notarization ticket onto Lattice.app";
            build-dmg = "Build UDZO DMG from stapled app";
            verify-gatekeeper = "spctl + codesign verify Gatekeeper path";
            latticed = "Build and run local latticed (debug)";
            agentd = "Build and run local lattice-agentd (debug)";
            ok = "No-op success (nxr task DAG join)";
          };

          scripts = {
            test = ''
              exec cargo test --workspace "$@"
            '';
            rust-test = ''
              exec cargo test --workspace "$@"
            '';
            rust-fmt-check = ''
              exec cargo fmt --all --check "$@"
            '';
            rust-clippy = ''
              exec cargo clippy --workspace --all-targets -- -D warnings "$@"
            '';
            lint = ''
              cargo clippy --workspace --all-targets -- -D warnings
              cargo fmt --all --check
            '';
            fmt = ''
              exec cargo fmt --all "$@"
            '';
            # Escape hatch — prefer `nxr task ci` (parallel DAG).
            check = ''
              cargo fmt --all --check
              cargo clippy --workspace --all-targets -- -D warnings
              cargo test --workspace
              pnpm install --frozen-lockfile --prefer-offline
              pnpm --filter @lattice/desktop build
            '';
            # Single install for NXR graphs. Leaf validation apps assume node_modules.
            js-deps = ''
              exec pnpm install --frozen-lockfile --prefer-offline "$@"
            '';
            # Direct `nix run` installs only if js-deps / prior install is missing.
            desktop-ui-test = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              exec pnpm --filter @lattice/desktop test "$@"
            '';
            generated-theme-check = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              pnpm --filter @lattice/desktop compile-theme
              git diff --exit-code -- \
                apps/desktop/src/theme-tokens.css \
                apps/desktop/src/theme-tokens.ts
            '';
            generated-template-check = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              pnpm compile-templates
              git diff --exit-code -- \
                crates/lattice-core/src/template_catalog.generated.rs \
                apps/desktop/src/templateCatalog.generated.ts \
                apps/desktop/src/demoWorkspace.generated.ts
            '';
            flake-check = ''
              exec nix flake check -L "$@"
            '';

            compile-theme = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              exec pnpm --filter @lattice/desktop compile-theme "$@"
            '';
            compile-templates = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              exec pnpm compile-templates "$@"
            '';
            prepare-first-look = ''
              exec bash scripts/prepare-first-look.sh "$@"
            '';
            desktop-dev = ''
              pnpm install --prefer-offline
              # Auto-load ecosystem secrets/ai.env when keys are missing so Tauri
              # does not silently fall back to LATTICE_AGENT_FAKE=1.
              exec bash scripts/exec-with-ai-env.sh \
                pnpm --filter @lattice/desktop tauri:dev "$@"
            '';
            desktop-web = ''
              pnpm install --prefer-offline
              exec bash scripts/exec-with-ai-env.sh \
                pnpm --filter @lattice/desktop dev -- --host 127.0.0.1 --port 5173 "$@"
            '';
            desktop-perf = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              pnpm --filter @lattice/desktop exec playwright install chromium
              exec pnpm --filter @lattice/desktop test:perf "$@"
            '';
            desktop-perf-tauri = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              exec pnpm --filter @lattice/desktop test:perf:tauri "$@"
            '';
            desktop = ''
              pnpm install --prefer-offline
              if [ ! -f apps/desktop/dist/index.html ]; then
                echo "lattice-desktop: building frontend into apps/desktop/dist…"
                pnpm --filter @lattice/desktop build
              else
                echo "lattice-desktop: reusing apps/desktop/dist (rebuild with: pnpm --filter @lattice/desktop build)"
              fi
              exec pnpm --filter @lattice/desktop exec tauri dev --config '{"build":{"beforeDevCommand":""}}' "$@"
            '';
            desktop-build = ''
              pnpm install --frozen-lockfile --prefer-offline
              # Match desktop-dev on macOS so release binaries include voice capture.
              # Linux CI stays featureless (no Swift FluidAudio bridges).
              if [ "$(uname -s)" = "Darwin" ]; then
                exec pnpm --filter @lattice/desktop exec tauri build --no-bundle --features voice-embedded "$@"
              else
                exec pnpm --filter @lattice/desktop tauri build --no-bundle "$@"
              fi
            '';
            desktop-ui-build = ''
              if [ ! -d node_modules ]; then
                pnpm install --frozen-lockfile --prefer-offline
              fi
              exec pnpm --filter @lattice/desktop build "$@"
            '';
            latticed = ''
              cargo build -p lattice-daemon --bin latticed
              exec target/debug/latticed "$@"
            '';
            agentd = ''
              cargo build -p lattice-agentd --bin lattice-agentd
              exec target/debug/lattice-agentd "$@"
            '';
            desktop-install = ''
              if [ "$(uname -s)" != "Darwin" ]; then
                echo "desktop-install: macOS only" >&2
                exit 1
              fi

              # Support `nix run ./lattice#…` from ecosystem root (Cargo workspace is nested).
              if [ -f ./lattice/Cargo.toml ] && [ -d ./lattice/apps/daemon ]; then
                cd ./lattice
              elif [ ! -f ./Cargo.toml ] || [ ! -d ./apps/daemon ]; then
                echo "desktop-install: run from lattice repo root (or ecosystem root with ./lattice)" >&2
                exit 1
              fi

              : "''${APPLE_SIGNING_IDENTITY:?Set APPLE_SIGNING_IDENTITY (see .env.example / docs/dev/environment.md)}"

              if [ -z "''${APPLE_TEAM_ID:-}" ]; then
                echo "desktop-install: warning: APPLE_TEAM_ID unset (ok for local Apple Development; needed later for notarization)" >&2
              fi

              pnpm install --frozen-lockfile --prefer-offline
              # Keep the Nix apple-sdk DEVELOPER_DIR/SDKROOT for the Cargo build.
              # Overriding to Xcode.app here mixes Xcode's MacOSX.sdk headers with
              # Nix libcxx and breaks libduckdb-sys (uint8_t / intmax_t / _CTYPE_*).
              # Same voice path as `nxr desktop-dev` / `pnpm tauri:dev` — without this,
              # Settings → Voice reports Unavailable (Cargo default features are empty).
              pnpm --filter @lattice/desktop exec tauri build --bundles app --features voice-embedded

              # Thin-client sidecars (semantic + voice + agent) must sit beside lattice-desktop.
              echo "desktop-install: building latticed / lattice-agentd / lattice-wasi-seatbelt / lattice-embed-host / lattice-voice-host"
              cargo build --release -p lattice-daemon --bin latticed
              cargo build --release -p lattice-agentd --bin lattice-agentd
              cargo build --release -p lattice-agentd --bin lattice-wasi-seatbelt
              # Align llama-cpp cmake with Nix apple-sdk (avoids MTLResidencySetDescriptor
              # link failures when cmake picks Xcode 26.x headers).
              # Runtime path relative to repo cwd; not a writeShellApplication input.
              # shellcheck disable=SC1091
              . scripts/macos/llama-cpp-nix-sdk.sh
              cargo build --release -p lattice-embed-host --bin lattice-embed-host --features llama-cpp
              cargo build --release -p lattice-voice-host --bin lattice-voice-host --features fluidaudio || \
                cargo build --release -p lattice-voice-host --bin lattice-voice-host

              echo "desktop-install: verifying production sidecars"
              for bin in latticed lattice-agentd lattice-wasi-seatbelt lattice-embed-host lattice-voice-host; do
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
              for dylib in libLatticeVoiceBridge.dylib libLatticeAudioBridge.dylib libLatticeApprovalBridge.dylib libLatticeAppleSignInBridge.dylib; do
                src="target/release/$dylib"
                if [ -f "$src" ]; then
                  cp -f "$src" "$macos_dir/$dylib"
                  echo "desktop-install: bundled $dylib"
                else
                  echo "desktop-install: warning: missing $src" >&2
                fi
              done

              # Quick Look appex (best-effort; requires Xcode).
              appex_out="$PWD/target/macos/LatticeQuickLook.appex"
              if bash scripts/macos/build-quicklook-appex.sh "$appex_out"; then
                mkdir -p "$app_src/Contents/PlugIns"
                rm -rf "$app_src/Contents/PlugIns/LatticeQuickLook.appex"
                cp -R "$appex_out" "$app_src/Contents/PlugIns/LatticeQuickLook.appex"
                echo "desktop-install: bundled LatticeQuickLook.appex"
              fi

              # Semantic search + voice + agent thin-clients expect sidecars
              # as MacOS siblings of the app binary (see docs/search/…).
              for bin in latticed lattice-agentd lattice-wasi-seatbelt lattice-embed-host lattice-voice-host; do
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
              # Match desktop-release: per-binary hardened runtime + entitlements, then
              # notarize + staple. Re-signing after Tauri's notarize invalidates the ticket;
              # Gatekeeper then SIGKILLs CLI/Finder launches (spctl: Unnotarized Developer ID).
              bash scripts/release/codesign-app.sh
              if [ -n "''${APPLE_ID:-}" ] && [ -n "''${APPLE_PASSWORD:-}" ] && [ -n "''${APPLE_TEAM_ID:-}" ]; then
                bash scripts/release/notarize-app.sh
                bash scripts/release/staple-app.sh
              else
                echo "desktop-install: warning: APPLE_ID/PASSWORD/TEAM_ID unset — skipping notarize; Gatekeeper may kill the app" >&2
              fi

              dest="''${LATTICE_INSTALL_DIR:-/Applications}/Lattice.app"
              echo "desktop-install: installing → $dest"
              rm -rf "$dest"
              ditto "$app_src" "$dest"
              codesign -dv --verbose=2 "$dest" || true
              if command -v spctl >/dev/null 2>&1; then
                spctl --assess --verbose=4 --type execute "$dest" || true
              fi
              echo "desktop-install: done. Open with: open \"$dest\""
              echo "desktop-install: for OpenAI agent env, do not use Finder/open alone — from ecosystem root:"
              echo "  ./scripts/exec-for-dev.sh -- \"$dest/Contents/MacOS/lattice-desktop\""
            '';
            # Distribution DAG leaves live under scripts/release/.
            # `nxr task desktop-release` orchestrates; apple-release context only on sign/notary.
            release-env-validate = ''
              exec bash scripts/release/env-validate.sh "$@"
            '';
            desktop-tauri-bundle = ''
              exec bash scripts/release/tauri-bundle.sh "$@"
            '';
            build-latticed = ''
              exec bash scripts/release/build-sidecar.sh lattice-daemon latticed
            '';
            build-agentd = ''
              exec bash scripts/release/build-sidecar.sh lattice-agentd lattice-agentd
              exec bash scripts/release/build-sidecar.sh lattice-agentd lattice-wasi-seatbelt
            '';
            build-embed-host = ''
              exec bash scripts/release/build-sidecar.sh lattice-embed-host lattice-embed-host llama-cpp
            '';
            build-voice-host = ''
              exec bash scripts/release/build-sidecar.sh lattice-voice-host lattice-voice-host fluidaudio
            '';
            verify-sidecars = ''
              exec bash scripts/release/verify-sidecars.sh "$@"
            '';
            assemble-app = ''
              exec bash scripts/release/assemble-app.sh "$@"
            '';
            codesign-app = ''
              exec bash scripts/release/codesign-app.sh "$@"
            '';
            notarize-app = ''
              exec bash scripts/release/notarize-app.sh "$@"
            '';
            staple-app = ''
              exec bash scripts/release/staple-app.sh "$@"
            '';
            build-dmg = ''
              exec bash scripts/release/build-dmg.sh "$@"
            '';
            verify-gatekeeper = ''
              exec bash scripts/release/verify-gatekeeper.sh "$@"
            '';
            # Thin pointer kept so `nix run .#desktop-release` still works; prefer the task DAG.
            desktop-release = ''
              echo "desktop-release: use the NXR DAG (secrets only on sign/notary):" >&2
              echo "  nxr task desktop-release" >&2
              echo "  # validate only: LATTICE_RELEASE_VALIDATE_ONLY=1 nxr task release-env-validate" >&2
              exec nix run .#nxr -- task desktop-release "$@"
            '';
            desktop-release-internal = ''
              exec bash scripts/release/build-internal-channel.sh "$@"
            '';
            ok = ''
              true
            '';
          };

          runtimeInputsFor =
            name:
            if name == "flake-check" then
              toolchain ++ [
                pkgs.nix
                pkgs.git
              ]
            else if
              builtins.elem name [
                "generated-theme-check"
                "generated-template-check"
              ]
            then
              toolchain ++ [ pkgs.git ]
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

          defaultLatticeScripts = latticeScripts;
        in
        {
          packages.nxr = nxr.packages.${system}.nxr;

          nxr.shellIntegration = {
            enable = true;
            devShells = [ "default" ];
          };

          # Secrets are delivered only to tasks that declare a context — not via .envrc.
          nxr.contexts = {
            agent-openai = {
              environment = {
                mode = "inherit";
                unset = [
                  "PIONEER_API_KEY"
                  "APPLE_ID"
                  "APPLE_PASSWORD"
                  "APPLE_TEAM_ID"
                  "APPLE_SIGNING_IDENTITY"
                  "CLOUDFLARE_API_TOKEN"
                ];
              };
              secrets = {
                OPENAI_API_KEY = {
                  ref = "OPENAI_API_KEY";
                  provider = "env";
                  delivery = "env";
                };
              };
            };
            agent-pioneer = {
              environment = {
                mode = "inherit";
                unset = [
                  "OPENAI_API_KEY"
                  "APPLE_ID"
                  "APPLE_PASSWORD"
                  "APPLE_TEAM_ID"
                  "APPLE_SIGNING_IDENTITY"
                  "CLOUDFLARE_API_TOKEN"
                ];
              };
              secrets = {
                PIONEER_API_KEY = {
                  ref = "PIONEER_API_KEY";
                  provider = "env";
                  delivery = "env";
                };
              };
            };
            apple-development = {
              environment = {
                mode = "inherit";
              };
              secrets = {
                APPLE_SIGNING_IDENTITY = {
                  ref = "APPLE_SIGNING_IDENTITY";
                  provider = "env";
                  delivery = "env";
                };
                APPLE_TEAM_ID = {
                  ref = "APPLE_TEAM_ID";
                  provider = "env";
                  delivery = "env";
                };
              };
            };
            apple-release = {
              environment = {
                mode = "clean";
                keep = [
                  "HOME"
                  "PATH"
                  "TMPDIR"
                  "SSH_AUTH_SOCK"
                  "USER"
                  "LANG"
                  "TERM"
                  "NIX_SSL_CERT_FILE"
                  "SSL_CERT_FILE"
                  "DEVELOPER_DIR"
                  "SDKROOT"
                  "LATTICE_RELEASE_VALIDATE_ONLY"
                  "LATTICE_INSTALL_DIR"
                ];
              };
              secrets = {
                APPLE_ID = {
                  ref = "APPLE_ID";
                  provider = "env";
                  delivery = "env";
                };
                APPLE_PASSWORD = {
                  ref = "APPLE_PASSWORD";
                  provider = "env";
                  delivery = "env";
                };
                APPLE_TEAM_ID = {
                  ref = "APPLE_TEAM_ID";
                  provider = "env";
                  delivery = "env";
                };
                APPLE_SIGNING_IDENTITY = {
                  ref = "APPLE_SIGNING_IDENTITY";
                  provider = "env";
                  delivery = "env";
                };
              };
              confirm = true;
            };

            # Local AI + local lattice-server (no provider keys baked in).
            # Load AI keys via exec-with-ai-env / sops; Finder launches need a
            # baked channel (see desktop-release-internal), not shell export.
            dev-local-ai = {
              environment = {
                mode = "inherit";
                set = {
                  LATTICE_CLOUD_URL = "http://127.0.0.1:8788";
                  LATTICE_AI_POLICY = "local";
                };
                unset = [
                  "APPLE_ID"
                  "APPLE_PASSWORD"
                  "APPLE_TEAM_ID"
                  "APPLE_SIGNING_IDENTITY"
                  "CLOUDFLARE_API_TOKEN"
                ];
              };
              secrets = {
                PIONEER_API_KEY = {
                  ref = "PIONEER_API_KEY";
                  provider = "env";
                  delivery = "env";
                };
                OPENAI_API_KEY = {
                  ref = "OPENAI_API_KEY";
                  provider = "env";
                  delivery = "env";
                };
              };
            };

            # Production cloud URL + AI keys from env (still not baked into DMG).
            dev-cloud-ai = {
              environment = {
                mode = "inherit";
                set = {
                  LATTICE_CLOUD_URL = "https://cloud.lattice-notes.com";
                  LATTICE_AI_POLICY = "cloud";
                };
                unset = [
                  "APPLE_ID"
                  "APPLE_PASSWORD"
                  "APPLE_TEAM_ID"
                  "APPLE_SIGNING_IDENTITY"
                  "CLOUDFLARE_API_TOKEN"
                ];
              };
              secrets = {
                PIONEER_API_KEY = {
                  ref = "PIONEER_API_KEY";
                  provider = "env";
                  delivery = "env";
                };
                OPENAI_API_KEY = {
                  ref = "OPENAI_API_KEY";
                  provider = "env";
                  delivery = "env";
                };
              };
            };
          };

          nxr.processes = {
            desktop-web = {
              app = "desktop-web";
              readiness = {
                http = {
                  url = "http://127.0.0.1:5173";
                };
              };
              restart = "on-failure";
            };
            latticed = {
              app = "latticed";
              restart = "on-failure";
            };
            agentd = {
              app = "agentd";
              dependsOn = [ "latticed" ];
              restart = "on-failure";
            };
          };

          nxr.apps = lib.mapAttrs (name: script: {
            description = descriptions.${name};
            runtimeInputs = runtimeInputsFor name;
            inherit script;
          }) scripts;

          # Orchestration around flake apps. Leaf apps stay authoritative;
          # `nxr task` / `nxr graph` use this metadata.
          # `paths` = affected ownership; `inputs.paths` = workspace-cache identity.
          nxr.tasks =
            let
              rustPaths = [
                "Cargo.toml"
                "Cargo.lock"
                "apps/**/*.rs"
                "crates/**/*.rs"
              ];
              desktopUiPaths = [
                "apps/desktop/**"
                "packages/**"
                "pnpm-lock.yaml"
                "package.json"
                "pnpm-workspace.yaml"
              ];
              cargoLock = {
                cpu = 2;
                memory = "4GiB";
                exclusive = [ "cargo-target" ];
              };
              pnpmLock = {
                exclusive = [ "pnpm-install" ];
              };
            in
            {
              test = {
                description = "Run cargo tests";
                app = "test";
                category = "validation";
                paths = rustPaths;
                resources = cargoLock;
              };
              rust-test = {
                description = "Run cargo tests";
                app = "rust-test";
                category = "validation";
                paths = rustPaths;
                resources = cargoLock;
              };
              rust-fmt-check = {
                description = "cargo fmt --check";
                app = "rust-fmt-check";
                category = "validation";
                paths = rustPaths;
              };
              rust-clippy = {
                description = "cargo clippy -D warnings";
                app = "rust-clippy";
                category = "validation";
                paths = rustPaths;
                resources = cargoLock;
              };
              lint = {
                description = "Clippy + rustfmt check (compat)";
                app = "lint";
                category = "validation";
                paths = rustPaths;
                resources = cargoLock;
              };
              fmt = {
                description = "Format Rust sources";
                app = "fmt";
                category = "development";
                paths = rustPaths;
              };
              # Escape hatch — prefer `nxr task ci`.
              check = {
                description = "Monolithic CI gate (escape hatch)";
                app = "check";
                category = "validation";
              };
              js-deps = {
                description = "Frozen pnpm install (shared by JS leaves)";
                app = "js-deps";
                category = "development";
                paths = [
                  "package.json"
                  "pnpm-lock.yaml"
                  "pnpm-workspace.yaml"
                  "apps/desktop/package.json"
                  "packages/**/package.json"
                ];
                resources = {
                  exclusive = [ "pnpm-install" ];
                };
              };
              ci = {
                description = "Authoritative CI DAG (parallel leaves)";
                app = "ok";
                dependsOn = [
                  "rust-fmt-check"
                  "rust-clippy"
                  "rust-test"
                  "desktop-ui-test"
                  "desktop-ui-build"
                  "generated-theme-check"
                  "generated-template-check"
                  "flake-check"
                ];
                category = "validation";
                aliases = [ "ci-fast" ];
              };
              validate = {
                description = "Fast parallel validation (lint ∥ test ∥ desktop UI)";
                dependsOn = [
                  "rust-clippy"
                  "rust-fmt-check"
                  "rust-test"
                  "desktop-ui-build"
                ];
                app = "ok";
                category = "validation";
              };
              desktop-ui-test = {
                description = "Desktop Vitest";
                app = "desktop-ui-test";
                dependsOn = [
                  "js-deps"
                ];
                category = "validation";
                paths = desktopUiPaths;
                resources = {
                  cpu = 2;
                  memory = "2GiB";
                };
              };
              generated-theme-check = {
                description = "Theme tokens match committed outputs";
                app = "generated-theme-check";
                dependsOn = [
                  "js-deps"
                ];
                category = "validation";
                paths = [
                  "themes/**"
                  "scripts/compile-theme.mjs"
                  "apps/desktop/src/theme-tokens.css"
                  "apps/desktop/src/theme-tokens.ts"
                ];
                resources = {
                  exclusive = [
                    "theme-generated"
                  ];
                };
              };
              generated-template-check = {
                description = "Template catalogs match committed outputs";
                app = "generated-template-check";
                dependsOn = [
                  "js-deps"
                ];
                category = "validation";
                paths = [
                  "templates/**"
                  "apps/desktop/src/templateCatalog.generated.ts"
                  "apps/desktop/src/demoWorkspace.generated.ts"
                  "crates/lattice-core/src/template_catalog.generated.rs"
                ];
                resources = {
                  exclusive = [
                    "template-generated"
                  ];
                };
              };
              flake-check = {
                description = "nix flake check";
                app = "flake-check";
                category = "validation";
                paths = [
                  "flake.nix"
                  "flake.lock"
                ];
              };
              compile-theme = {
                description = "Compile theme tokens";
                app = "compile-theme";
                dependsOn = [
                  "js-deps"
                ];
                category = "codegen";
                paths = [
                  "themes/**"
                  "scripts/compile-theme.mjs"
                ];
                inputs = {
                  paths = [
                    "themes/**"
                    "scripts/compile-theme.mjs"
                    "package.json"
                    "pnpm-lock.yaml"
                    "apps/desktop/package.json"
                  ];
                };
                outputs = [
                  {
                    path = "apps/desktop/src/theme-tokens.css";
                    mode = "verify-only";
                  }
                  {
                    path = "apps/desktop/src/theme-tokens.ts";
                    mode = "verify-only";
                  }
                ];
                # Local CAS only — never enable cache on secret-bearing contexts until
                # NXR disables caching by default when context secrets are present.
                cache = {
                  mode = "local";
                  version = "1";
                };
                resources = {
                  exclusive = [
                    "theme-generated"
                  ];
                };
              };
              compile-templates = {
                description = "Compile workspace templates";
                app = "compile-templates";
                dependsOn = [
                  "js-deps"
                ];
                category = "codegen";
                paths = [ "templates/**" ];
                inputs = {
                  paths = [
                    "templates/**"
                    "package.json"
                    "pnpm-lock.yaml"
                  ];
                };
                outputs = [
                  {
                    path = "crates/lattice-core/src/template_catalog.generated.rs";
                    mode = "verify-only";
                  }
                  {
                    path = "apps/desktop/src/templateCatalog.generated.ts";
                    mode = "verify-only";
                  }
                  {
                    path = "apps/desktop/src/demoWorkspace.generated.ts";
                    mode = "verify-only";
                  }
                ];
                # Local CAS only — never enable cache on secret-bearing contexts until
                # NXR disables caching by default when context secrets are present.
                cache = {
                  mode = "local";
                  version = "1";
                };
                resources = {
                  exclusive = [
                    "template-generated"
                  ];
                };
              };
              prepare-first-look = {
                description = "Seed First Look demo datasets and regenerate catalogs";
                app = "prepare-first-look";
                category = "development";
                aliases = [ "prep-demo" ];
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

              desktop-dev = {
                description = "Tauri + Vite HMR";
                app = "desktop-dev";
                category = "development";
                resources = {
                  exclusive = [
                    "cargo-target"
                    "pnpm-install"
                  ];
                };
              };
              desktop-web = {
                description = "Browser-only demo UI";
                app = "desktop-web";
                category = "development";
                resources = pnpmLock;
              };
              desktop = {
                description = "Native without Vite";
                app = "desktop";
                category = "development";
              };
              desktop-build = {
                description = "Unbundled release binary";
                app = "desktop-build";
                category = "development";
                resources = {
                  cpu = 4;
                  memory = "8GiB";
                  exclusive = [
                    "cargo-target"
                    "pnpm-install"
                  ];
                };
              };
              desktop-ui-build = {
                description = "Build desktop frontend (Vite)";
                app = "desktop-ui-build";
                dependsOn = [
                  "js-deps"
                ];
                category = "validation";
                paths = desktopUiPaths;
                resources = {
                  cpu = 2;
                  memory = "4GiB";
                };
              };
              desktop-install = {
                description = "Sign and install Lattice.app locally (macOS)";
                app = "desktop-install";
                category = "release";
                aliases = [ "install" ];
                context = "apple-development";
                resources = {
                  cpu = 4;
                  memory = "8GiB";
                  exclusive = [
                    "cargo-target"
                    "pnpm-install"
                    "xcode-derived-data"
                    "apple-keychain"
                  ];
                };
              };

              # Release DAG: compile without Apple secrets; sign/notary use apple-release.
              release-env-validate = {
                description = "Validate Apple Developer ID + notarytool env";
                app = "release-env-validate";
                category = "release";
                context = "apple-release";
              };
              desktop-tauri-bundle = {
                description = "Tauri app bundle (voice-embedded)";
                app = "desktop-tauri-bundle";
                category = "release";
                dependsOn = [
                  "js-deps"
                  "release-env-validate"
                ];
                resources = {
                  cpu = 4;
                  memory = "8GiB";
                  exclusive = [
                    "cargo-target"
                    "xcode-derived-data"
                  ];
                };
              };
              build-latticed = {
                description = "Release-build latticed";
                app = "build-latticed";
                category = "release";
                dependsOn = [ "release-env-validate" ];
                resources = {
                  exclusive = [ "cargo-target" ];
                };
              };
              build-agentd = {
                description = "Release-build lattice-agentd";
                app = "build-agentd";
                category = "release";
                dependsOn = [ "release-env-validate" ];
                resources = {
                  exclusive = [ "cargo-target" ];
                };
              };
              build-embed-host = {
                description = "Release-build lattice-embed-host";
                app = "build-embed-host";
                category = "release";
                dependsOn = [ "release-env-validate" ];
                resources = {
                  exclusive = [ "cargo-target" ];
                };
              };
              build-voice-host = {
                description = "Release-build lattice-voice-host";
                app = "build-voice-host";
                category = "release";
                dependsOn = [ "release-env-validate" ];
                resources = {
                  exclusive = [ "cargo-target" ];
                };
              };
              verify-sidecars = {
                description = "Verify release sidecars";
                app = "verify-sidecars";
                category = "release";
                dependsOn = [
                  "build-latticed"
                  "build-agentd"
                  "build-embed-host"
                  "build-voice-host"
                ];
              };
              assemble-app = {
                description = "Assemble sidecars into Lattice.app";
                app = "assemble-app";
                category = "release";
                dependsOn = [
                  "desktop-tauri-bundle"
                  "verify-sidecars"
                ];
              };
              codesign-app = {
                description = "Developer ID codesign (hardened runtime)";
                app = "codesign-app";
                category = "release";
                dependsOn = [ "assemble-app" ];
                context = "apple-release";
                resources = {
                  exclusive = [
                    "apple-keychain"
                    "xcode-derived-data"
                  ];
                };
              };
              notarize-app = {
                description = "Apple notarytool submit --wait";
                app = "notarize-app";
                category = "release";
                dependsOn = [ "codesign-app" ];
                context = "apple-release";
                resources = {
                  network = true;
                  exclusive = [
                    "apple-keychain"
                    "apple-notary"
                  ];
                };
              };
              staple-app = {
                description = "Staple notarization ticket";
                app = "staple-app";
                category = "release";
                dependsOn = [ "notarize-app" ];
                resources = {
                  exclusive = [ "apple-notary" ];
                };
              };
              build-dmg = {
                description = "Build UDZO DMG";
                app = "build-dmg";
                category = "release";
                dependsOn = [ "staple-app" ];
              };
              verify-gatekeeper = {
                description = "Gatekeeper / codesign verify";
                app = "verify-gatekeeper";
                category = "release";
                dependsOn = [ "build-dmg" ];
              };
              desktop-release = {
                description = "Notarized macOS DMG DAG";
                app = "ok";
                dependsOn = [ "verify-gatekeeper" ];
                category = "release";
                aliases = [ "release" ];
              };

              # Side-by-side internal channel (bundle id dev.lattice.desktop.dev).
              # Reuses apple-release Developer ID context; live notarize is optional.
              desktop-release-internal = {
                description = "Internal-channel app build (staging cloud URL baked via env)";
                app = "desktop-release-internal";
                category = "release";
                context = "apple-release";
                dependsOn = [ "js-deps" ];
                resources = {
                  exclusive = [
                    "apple-keychain"
                    "xcode-derived-data"
                    "cargo-target"
                  ];
                };
              };

              desktop-perf = {
                description = "Browser perf harness";
                app = "desktop-perf";
                dependsOn = [
                  "js-deps"
                ];
                category = "validation";
              };
              desktop-perf-tauri = {
                description = "Tauri WebView perf harness";
                app = "desktop-perf-tauri";
                dependsOn = [
                  "js-deps"
                ];
                category = "validation";
              };
              latticed = {
                description = "Run local latticed";
                app = "latticed";
                category = "development";
                resources = {
                  exclusive = [ "cargo-target" ];
                };
              };
              agentd = {
                description = "Run local lattice-agentd";
                app = "agentd";
                category = "development";
                context = "agent-pioneer";
                resources = {
                  exclusive = [ "cargo-target" ];
                };
              };
            };
          # Day-to-day Rust/desktop. Site/Cloudflare live in private lattice-ecosystem.
          devShells.default = pkgs.mkShell {
            packages = toolchain ++ lib.attrValues defaultLatticeScripts;
            shellHook = ''
              export RUSTC_WRAPPER="''${RUSTC_WRAPPER:-sccache}"
              if [ "$(uname -s)" = "Darwin" ]; then
                export SCCACHE_DIR="''${SCCACHE_DIR:-''$HOME/Library/Caches/Lattice/sccache}"
              else
                export SCCACHE_DIR="''${SCCACHE_DIR:-''${XDG_CACHE_HOME:-''$HOME/.cache}/lattice/sccache}"
              fi
              if [ "''${NXR_AUTO_DAEMON:-1}" != "0" ] && command -v nxr >/dev/null 2>&1; then
                nxr daemon status --json >/dev/null 2>&1 ||
                  nxr daemon start >/dev/null 2>&1 ||
                  true
              fi
              echo "lattice dev shell — rust $(rustc --version | cut -d' ' -f2), node $(node --version), pnpm $(pnpm --version)"
              echo "runner: nxr list | nxr task ci [-j N] | nxr graph ci | nxr up desktop-web"
              echo "release: nxr graph desktop-release | nxr task desktop-release-internal"
              echo "contexts: nxr context list (dev-local-ai, dev-cloud-ai, agent-*, apple-*)"
              echo "site / Cloudflare: private lattice-ecosystem (not this flake)"
            '';
          };
        };
    };
}
