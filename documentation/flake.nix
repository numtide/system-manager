{
  description = "System Manager docs environment (MkDocs + Material)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Pick a Python; 3.12 is a nice current default.
        python = pkgs.python312;

        # Bundle mkdocs + material + common plugins into one Python env
        pyEnv = python.withPackages (
          ps: with ps; [
            mkdocs
            mkdocs-material
            pymdown-extensions
            mkdocs-git-revision-date-localized-plugin
            mkdocs-awesome-nav
          ]
        );
      in
      {
        # `nix develop` or direnv will use this
        devShells.default = pkgs.mkShell {
          packages = [ pyEnv ];
          shellHook = ''
            echo "📚 MkDocs ready. Try: mkdocs serve --config-file mkdocs.yml"
          '';
        };

      }
    );
}
