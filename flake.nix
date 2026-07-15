{
  description = "Lattice — local-first open-native workspace (dev shell)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer

            # JS toolchain (desktop shell + site)
            nodejs_22
            pnpm

            # Build prerequisites
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

          shellHook = ''
            echo "lattice dev shell — rust $(rustc --version | cut -d' ' -f2), node $(node --version), pnpm $(pnpm --version)"
          '';
        };
      });
    };
}
