{
  description = "Issue workflow and repository automation tools";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        tools = pkgs.rustPlatform.buildRustPackage {
          pname = "tools";
          version = cargoToml.workspace.package.version;
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          buildAndTestSubdir = "crates/cli";
        };
      in rec {
        packages.tools = tools;
        packages.default = tools;

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            gh
            jq
            just
            lefthook
            # `just workflows-check`: workflow lint (actionlint runs shellcheck on `run:` blocks)
            # and scripts/release-tag.sh with its harness.
            actionlint
            shellcheck
            python3
            git
          ];
          # Checked by the justfile's `_devshell` guard: recipes run only with this toolchain.
          TOOLS_DEVSHELL = "1";
        };
      }
    );
}
