{
  nixpkgs ? <nixpkgs>,
  lib ? import "${nixpkgs}/lib",
  nixos ? "${nixpkgs}/nixos",
}:
rec {
  # Function that can be used when defining inline modules to get better location
  # reporting in module-system errors.
  # Usage example:
  #   { _file = "${printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module"; }
  printAttrPos =
    {
      file,
      line,
      column,
    }:
    "${file}:${toString line}:${toString column}";

  makeSystemConfig =
    {
      modules,
      extraSpecialArgs ? { },
    }:
    let
      # Module that sets additional module arguments
      extraArgsModule =
        {
          lib,
          config,
          pkgs,
          ...
        }:
        {
          _file = "${printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module";
          _module.args = {
            pkgs = import nixpkgs { system = config.nixpkgs.hostPlatform; };
            utils = import "${nixos}/lib/utils.nix" {
              inherit lib config pkgs;
            };
            # Pass the wrapped system-manager binary down
            # TODO: Use nixpkgs version by default.
            inherit
              (import ../packages.nix { pkgs = import nixpkgs { system = config.nixpkgs.hostPlatform; }; })
              system-manager
              ;
          };
        };

      config =
        (lib.evalModules {
          specialArgs = {
            nixosModulesPath = "${nixos}/modules";
          } // extraSpecialArgs;
          modules = [
            extraArgsModule
            ./modules
          ] ++ modules;
        }).config;

      # Get the system as it was defined in the modules.
      system = config.nixpkgs.hostPlatform;
      pkgs = import nixpkgs { inherit system; };

      returnIfNoAssertions =
        drv:
        let
          failedAssertions = map (x: x.message) (lib.filter (x: !x.assertion) config.assertions);
        in
        if failedAssertions != [ ] then
          throw "\nFailed assertions:\n${lib.concatStringsSep "\n" (map (x: "- ${x}") failedAssertions)}"
        else
          lib.showWarnings config.warnings drv;

      servicesPath = pkgs.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } config.build.services;
      };

      etcPath = pkgs.writeTextFile {
        name = "etcFiles";
        destination = "/etcFiles.json";
        text = lib.generators.toJSON { } { inherit (config.build.etc) entries staticEnv; };
      };

      linkFarmNestedEntryFromDrv = dirs: drv: {
        name = lib.concatStringsSep "/" (dirs ++ [ "${drv.name}" ]);
        path = drv;
      };
      linkFarmEntryFromDrv = linkFarmNestedEntryFromDrv [ ];
      linkFarmBinEntryFromDrv = linkFarmNestedEntryFromDrv [ "bin" ];

      toplevel =
        let
          scripts = lib.mapAttrsToList (_: script: linkFarmBinEntryFromDrv script) config.build.scripts;

          entries = [
            (linkFarmEntryFromDrv servicesPath)
            (linkFarmEntryFromDrv etcPath)
          ] ++ scripts;

          addPassthru =
            drv:
            drv.overrideAttrs (prevAttrs: {
              passthru = (prevAttrs.passthru or { }) // {
                inherit config;
              };
            });
        in
        addPassthru (pkgs.linkFarm "system-manager" entries);
    in
    returnIfNoAssertions toplevel;

  mkTestPreamble =
    {
      node,
      profile,
      action,
    }:
    ''
      ${node}.succeed("${profile}/bin/${action} 2>&1 | tee /tmp/output.log")
      ${node}.succeed("! grep -F 'ERROR' /tmp/output.log")
    '';

  activateProfileSnippet =
    { node, profile }:
    mkTestPreamble {
      inherit node profile;
      action = "activate";
    };
  deactivateProfileSnippet =
    { node, profile }:
    mkTestPreamble {
      inherit node profile;
      action = "deactivate";
    };
  prepopulateProfileSnippet =
    { node, profile }:
    mkTestPreamble {
      inherit node profile;
      action = "prepopulate";
    };
}
