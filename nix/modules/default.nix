{
  lib,
  config,
  pkgs,
  system-manager,
  ...
}:
{
  imports = [
    ./environment.nix
    ./etc.nix
    ./systemd.nix
    ./tmpfiles.nix
    ./upstream/nixpkgs
  ];

  options =
    let
      inherit (lib) types;
    in
    {
      nixpkgs = {
        buildPlatform = lib.mkOption {
          type = types.str;
          example = "x86_64-linux";
          default = config.nixpkgs.hostPlatform;
        };

        hostPlatform = lib.mkOption {
          type = with types; either str attrs;
          example = "x86_64-linux";
          default = throw "the option nixpkgs.hostPlatform needs to be set.";
        };

        overlays = lib.mkOption {
          type = with types; listOf anything;
          default = [ ];
        };

        config = lib.mkOption {
          type = types.attrs;
          description = ''Configuration used to instantiate nixpkgs.'';
          default = { };
        };

        pkgs = lib.mkOption {
          type = lib.types.pkgs;
          description = ''The pkgs module argument.'';
          default = pkgs;
          readOnly = true;
        };
      };

      assertions = lib.mkOption {
        type = types.listOf types.unspecified;
        internal = true;
        default = [ ];
        example = [
          {
            assertion = false;
            message = "you can't enable this for that reason";
          }
        ];
        description = lib.mdDoc ''
          This option allows modules to express conditions that must
          hold for the evaluation of the system configuration to
          succeed, along with associated error messages for the user.
        '';
      };

      warnings = lib.mkOption {
        internal = true;
        default = [ ];
        type = types.listOf types.str;
        example = [ "The `foo' service is deprecated and will go away soon!" ];
        description = lib.mdDoc ''
          This option allows modules to show warnings to users during
          the evaluation of the system configuration.
        '';
      };

      # Statically assigned UIDs and GIDs.
      # Ideally we use DynamicUser as much as possible to avoid the need for these.
      ids = {
        uids = lib.mkOption {
          internal = true;
          description = lib.mdDoc ''
            The user IDs used by system-manager.
          '';
          type = types.attrsOf types.int;
        };

        gids = lib.mkOption {
          internal = true;
          description = lib.mdDoc ''
            The group IDs used by system-manager.
          '';
          type = types.attrsOf types.int;
        };
      };

      # No-op option for now.
      # TODO: should we include the settings in /etc/logrotate.d ?
      services.logrotate = lib.mkOption {
        internal = true;
        default = { };
        type = types.attrs;
      };

      # No-op option for now.
      users = lib.mkOption {
        internal = true;
        default = { };
        type = types.attrs;
      };

      networking = {
        enableIPv6 = lib.mkEnableOption "IPv6" // {
          default = true;
        };
      };

      system-manager = {
        allowAnyDistro = lib.mkEnableOption "the usage of system-manager on untested distributions";

        preActivationAssertions = lib.mkOption {
          type =
            with lib.types;
            attrsOf (
              submodule (
                { name, ... }:
                {
                  options = {
                    enable = lib.mkEnableOption "the assertion";

                    name = lib.mkOption {
                      type = types.str;
                      default = name;
                    };

                    script = lib.mkOption {
                      type = types.str;
                    };
                  };
                }
              )
            );
          default = { };
        };
      };

      build = {
        toplevel = lib.mkOption {
          type = lib.types.pathInStore;
          readOnly = true;
        };

        scripts = lib.mkOption {
          type = lib.types.attrsOf lib.types.package;
        };

        etc = {
          staticEnv = lib.mkOption {
            type = lib.types.package;
          };

          entries = lib.mkOption {
            # TODO: better type
            type = lib.types.attrsOf lib.types.raw;
          };
        };

        services = lib.mkOption {
          # TODO: better type
          type = lib.types.attrsOf lib.types.raw;
        };
      };
    };

  config = {
    system-manager.preActivationAssertions = {
      osVersion =
        let
          supportedIds = [
            "nixos"
            "ubuntu"
          ];
        in
        {
          enable = !config.system-manager.allowAnyDistro;
          script = ''
            source /etc/os-release
            ${lib.concatStringsSep "\n" (
              lib.flip map supportedIds (supportedId: ''
                if [ $ID = "${supportedId}" ]; then
                  exit 0
                fi
              '')
            )}
            echo "This OS is not currently supported."
            echo "Supported OSs are: ${lib.concatStringsSep ", " supportedIds}"
            exit 1
          '';
        };
    };

    build = {
      scripts = {
        registerProfileScript = pkgs.writeShellScript "register-profile" ''
          ${system-manager}/bin/system-manager register \
            --store-path "$(dirname $(realpath $(dirname ''${0})))" \
            "$@"
        '';

        activationScript = pkgs.writeShellScript "activate" ''
          ${system-manager}/bin/system-manager activate \
            --store-path "$(dirname $(realpath $(dirname ''${0})))" \
            "$@"
        '';

        prepopulateScript = pkgs.writeShellScript "prepopulate" ''
          ${system-manager}/bin/system-manager pre-populate \
            --store-path "$(dirname $(realpath $(dirname ''${0})))" \
            "$@"
        '';

        deactivationScript = pkgs.writeShellScript "deactivate" ''
          ${system-manager}/bin/system-manager deactivate "$@"
        '';

        systemActivationScript = pkgs.writeShellScript "systemActivationScript" config.system.activationScripts.script;

        preActivationAssertionScript =
          let
            mkAssertion =
              { name, script, ... }:
              ''
                # ${name}

                echo -e "Evaluating pre-activation assertion ${name}...\n"
                (
                  set +e
                  ${script}
                )
                assertion_result=$?

                if [ $assertion_result -ne 0 ]; then
                  failed_assertions+=${name}
                fi
              '';

            mkAssertions =
              assertions:
              lib.concatStringsSep "\n" (
                lib.mapAttrsToList (name: mkAssertion) (lib.filterAttrs (name: cfg: cfg.enable) assertions)
              );
          in
          pkgs.writeShellScript "preActivationAssertions" ''
            set -ou pipefail

            declare -a failed_assertions=()

            ${mkAssertions config.system-manager.preActivationAssertions}

            if [ ''${#failed_assertions[@]} -ne 0 ]; then
              for failed_assertion in ''${failed_assertions[@]}; do
                echo "Pre-activation assertion $failed_assertion failed."
              done
              echo "See the output above for more details."
              exit 1
            else
              echo "All pre-activation assertions succeeded."
              exit 0
            fi
          '';

      };

      # TODO: handle globbing
      etc =
        let
          addToStore =
            name: file:
            pkgs.runCommandLocal "${name}-etc-link" { } ''
              mkdir -p "$out/$(dirname "${file.target}")"
              ln -s "${file.source}" "$out/${file.target}"

              if [ "${file.mode}" != symlink ]; then
                echo "${file.mode}" > "$out/${file.target}.mode"
                echo "${file.user}" > "$out/${file.target}.uid"
                echo "${file.group}" > "$out/${file.target}.gid"
              fi
            '';

          filteredEntries = lib.filterAttrs (_name: etcFile: etcFile.enable) config.environment.etc;

          srcDrvs = lib.mapAttrs addToStore filteredEntries;

          entries = lib.mapAttrs (name: file: file // { source = "${srcDrvs.${name}}"; }) filteredEntries;

          staticEnv = pkgs.buildEnv {
            name = "etc-static-env";
            paths = lib.attrValues srcDrvs;
          };
        in
        {
          inherit entries staticEnv;
        };

      services = lib.mapAttrs' (
        unitName: unit:
        lib.nameValuePair unitName {
          storePath = ''${unit.unit}/${unitName}'';
        }
      ) (lib.filterAttrs (_: unit: unit.enable) config.systemd.units);
    };
  };
}
