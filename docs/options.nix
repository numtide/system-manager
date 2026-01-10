# Generate module options documentation
#
# Build with: nix build .#docs.optionsCommonMark
# View JSON: nix build .#docs.optionsJSON && cat result/share/doc/nixos/options.json | jq
{
  pkgs,
  lib ? pkgs.lib,
}:
let
  # Add compatibility shim for lib.mdDoc (removed in recent nixpkgs)
  libCompat = lib.extend (
    final: prev: {
      mdDoc = x: x;
    }
  );

  # Evaluate modules to extract options
  # We need to provide minimal config that doesn't throw during option documentation generation
  eval = libCompat.evalModules {
    modules = [
      ../nix/modules
      # Set config values (not redeclare options)
      {
        config = {
          nixpkgs.hostPlatform = libCompat.mkDefault "x86_64-linux";
        };
      }
    ];
    specialArgs = {
      nixosModulesPath = "${pkgs.path}/nixos/modules";
      pkgs = pkgs;
      lib = libCompat;
      utils = import "${pkgs.path}/nixos/lib/utils.nix" {
        lib = libCompat;
        inherit pkgs;
        config = { };
      };
      system-manager = pkgs.hello; # Stub
    };
  };

  # Filter out internal and invisible options
  transformOptions =
    opt:
    opt
    // {
      # Strip the prefix from declaration paths for cleaner output
      declarations = map (
        decl:
        if lib.hasPrefix (toString ../.) (toString decl) then
          lib.removePrefix (toString ../. + "/") (toString decl)
        else
          decl
      ) opt.declarations;
    }
    # Remove problematic defaults that throw errors
    // lib.optionalAttrs (opt.name == "nixpkgs.hostPlatform") {
      default = {
        _type = "literalExpression";
        text = ''"x86_64-linux"'';
      };
    };

  optionsDoc = pkgs.nixosOptionsDoc {
    options = eval.options;
    inherit transformOptions;
    warningsAreErrors = false;
  };
in
{
  # Markdown format - can be used directly in MkDocs
  optionsCommonMark = optionsDoc.optionsCommonMark;

  # JSON format - for custom processing
  optionsJSON = optionsDoc.optionsJSON;

  # Nix attrset - for programmatic access
  optionsNix = optionsDoc.optionsNix;
}
