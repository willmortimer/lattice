{
  description = "Lattice — local-first open-native workspace (dev shell and tasks)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      toolchain = pkgs: with pkgs; [
        rustc
        cargo
        rustfmt
        clippy
        rust-analyzer
        nodejs_22
        pnpm
        pkg-config
      ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
        libiconv
      ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
        # Tauri 2 Linux prerequisites
        gtk3
        webkitgtk_4_1
        libayatana-appindicator
        librsvg
        openssl
      ];

      # A task is a shell script run with the full toolchain on PATH:
      #   nix run .#<name>
      mkTasks = pkgs: tasks:
        builtins.mapAttrs
          (name: script: {
            type = "app";
            program = pkgs.lib.getExe (pkgs.writeShellApplication {
              name = "lattice-${name}";
              runtimeInputs = toolchain pkgs;
              text = script;
            });
          })
          tasks;
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = toolchain pkgs;
          shellHook = ''
            echo "lattice dev shell — rust $(rustc --version | cut -d' ' -f2), node $(node --version), pnpm $(pnpm --version)"
            echo "tasks: nix run .#{test,lint,fmt,check,site-dev,site-build,desktop-dev,desktop-build,docs-sync}"
          '';
        };
      });

      apps = forAllSystems (pkgs: mkTasks pkgs {
        # Rust workspace
        test = "cargo test --workspace";
        lint = ''
          cargo clippy --workspace --all-targets -- -D warnings
          cargo fmt --all --check
        '';
        fmt = "cargo fmt --all";

        # Everything CI would run
        check = ''
          cargo fmt --all --check
          cargo clippy --workspace --all-targets -- -D warnings
          cargo test --workspace
          pnpm install --frozen-lockfile
          pnpm --filter @lattice/desktop build
          pnpm --filter @lattice/site build
        '';

        # Marketing/docs site
        site-dev = "pnpm install && pnpm --filter @lattice/site dev";
        site-build = "pnpm install && pnpm --filter @lattice/site build";
        docs-sync = "node site/scripts/sync-docs.mjs";

        # Desktop shell
        desktop-dev = "pnpm install && pnpm --filter @lattice/desktop tauri dev";
        desktop-build = "pnpm install && pnpm --filter @lattice/desktop tauri build --no-bundle";
      });
    };
}
