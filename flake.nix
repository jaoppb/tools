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
      in rec {
        packages.project-sync = pkgs.rustPlatform.buildRustPackage {
          pname = "project-sync";
          version = cargoToml.workspace.package.version;
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          buildAndTestSubdir = "crates/project-sync";
        };

        packages.issues-check = pkgs.rustPlatform.buildRustPackage {
          pname = "issues-check";
          version = cargoToml.workspace.package.version;
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          buildAndTestSubdir = "crates/issues-check";
        };

        packages.default = packages.project-sync;

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            gh
            just
          ];
        };
      }
    );
}
