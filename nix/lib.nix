{
  nixpkgs ? <nixpkgs>,
  lib ? import "${nixpkgs}/lib",
  nixos ? "${nixpkgs}/nixos",
}:
let
  self = {
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
        overlays ? [ ],
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
            _file = "${self.printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module";
            _module.args = {
              pkgs = let
                cfg = config.nixpkgs;
                systemArgs =
                  if cfg.buildPlatform != cfg.hostPlatform then
                    {
                      localSystem = cfg.buildPlatform;
                      crossSystem = cfg.hostPlatform;
                    }
                  else
                    {
                      system = cfg.hostPlatform;
                    };
                in
                import nixpkgs ({
                  overlays = overlays ++ cfg.overlays;
                  inherit (config.nixpkgs) config;
                } // systemArgs);
              utils = import "${nixos}/lib/utils.nix" {
                inherit lib config pkgs;
              };
              # Pass the wrapped system-manager binary down
              # TODO: Use nixpkgs version by default.
              inherit (import ../packages.nix { inherit pkgs; })
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
              {
                _file = "${self.printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module";
                build = { inherit toplevel; };
              }
            ] ++ modules;
          }).config;

        inherit (config.nixpkgs) pkgs;

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
      self.mkTestPreamble {
        inherit node profile;
        action = "activate";
      };
    deactivateProfileSnippet =
      { node, profile }:
      self.mkTestPreamble {
        inherit node profile;
        action = "deactivate";
      };
    prepopulateProfileSnippet =
      { node, profile }:
      self.mkTestPreamble {
        inherit node profile;
        action = "prepopulate";
      };
  };
in
self
