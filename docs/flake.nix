{
  description = "System Manager documentation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    mkdocs-numtide.url = "github:numtide/mkdocs-numtide";
    mkdocs-numtide.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      mkdocs-numtide,
    }:
    let
      systems = [
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      eachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f {
            inherit system;
            pkgs = nixpkgs.legacyPackages.${system};
          }
        );
    in
    {
      packages = eachSystem (
        { system, pkgs }:
        {
          default = pkgs.stdenvNoCC.mkDerivation {
            name = "system-manager-docs";

            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./site
                ./theme
                ./mkdocs.yml
              ];
            };

            nativeBuildInputs = [
              mkdocs-numtide.packages.${system}.default
            ];

            buildPhase = ''
              mkdocs build
            '';

            installPhase = ''
              mv out $out
            '';
          };
        }
      );

      devShells = eachSystem (
        { system, pkgs, ... }:
        {
          default = pkgs.mkShell {
            packages = [
              mkdocs-numtide.packages.${system}.default
            ];
            shellHook = ''
              echo "MkDocs ready. Try: mkdocs serve"
            '';
          };
        }
      );
    };
}
