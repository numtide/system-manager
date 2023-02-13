{ nixpkgs }:
let
  inherit (nixpkgs) lib;
in
{
  makeServiceConfig =
    { system
    , modules
    , system-manager
    ,
    }:
    let
      pkgs = nixpkgs.legacyPackages.${system};

      nixosConfig = (lib.nixosSystem {
        inherit system;
        modules = [ ./modules/system-manager.nix ] ++ modules;
        specialArgs = { };
      }).config;

      returnIfNoAssertions = drv:
        let
          failedAssertions = map (x: x.message) (lib.filter (x: !x.assertion) nixosConfig.assertions);
        in
        if failedAssertions != [ ]
        then throw "\nFailed assertions:\n${lib.concatStringsSep "\n" (map (x: "- ${x}") failedAssertions)}"
        else lib.showWarnings nixosConfig.warnings drv;

      services =
        lib.listToAttrs
          (map
            (name:
              let
                serviceName = "${name}.service";
              in
              lib.nameValuePair serviceName {
                storePath =
                  ''${nixosConfig.systemd.units."${serviceName}".unit}/${serviceName}'';
              })
            nixosConfig.system-manager.services);

      servicesPath = pkgs.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } services;
      };

      # TODO: handle globbing
      etcFiles =
        let
          isManaged = name: lib.elem name nixosConfig.system-manager.etcFiles;

          addToStore = name: file: pkgs.runCommandLocal "${name}-etc-link" { } ''
            mkdir -p "$out/etc/$(dirname "${file.target}")"
            ln -s "${file.source}" "$out/etc/${file.target}"

            if [ "${file.mode}" != symlink ]; then
              echo "${file.mode}" > "$out/etc/${file.target}.mode"
              echo "${file.user}" > "$out/etc/${file.target}.uid"
              echo "${file.group}" > "$out/etc/${file.target}.gid"
            fi
          '';

          filteredEntries = lib.filterAttrs
            (name: etcFile: etcFile.enable && isManaged name)
            nixosConfig.environment.etc;

          srcDrvs = lib.mapAttrs addToStore filteredEntries;

          entries = lib.mapAttrs
            (name: file: file // { source = "${srcDrvs.${name}}"; })
            filteredEntries;

          staticEnv = pkgs.buildEnv {
            name = "etc-static-env";
            paths = lib.attrValues srcDrvs;
          };
        in
        { inherit entries staticEnv; };

      etcPath = pkgs.writeTextFile {
        name = "etcFiles";
        destination = "/etcFiles.json";
        text = lib.generators.toJSON { } etcFiles;
      };

      # TODO: remove --ephemeral
      activationScript = pkgs.writeShellScript "activate" ''
        ${system-manager}/bin/system-manager activate \
          --store-path "$(realpath $(dirname ''${0}))" \
          "$@"
      '';
    in
    returnIfNoAssertions (
      pkgs.linkFarmFromDrvs "system-manager" [
        servicesPath
        etcPath
        activationScript
      ]
    );
}
