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
        mkPackage = pname: pkgs.rustPlatform.buildRustPackage {
          inherit pname;
          version = cargoToml.workspace.package.version;
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          buildAndTestSubdir = "crates/${pname}";
        };
      in rec {
        packages.project-sync = mkPackage "project-sync";
        packages.issues-check = mkPackage "issues-check";
        packages.repo-sync = mkPackage "repo-sync";
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
