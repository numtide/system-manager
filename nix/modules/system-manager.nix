{ lib
, config
, pkgs
, ...
}:
{
  options.system-manager = {
    services = lib.mkOption {
      type = with lib.types; listOf str;
      default = [ ];
    };

    etcFiles = lib.mkOption {
      type = with lib.types; listOf str;
      default = [ ];
    };

    allowAnyDistro = lib.mkEnableOption "the usage of system-manager on untested distributions";

    preActivationAssertions = lib.mkOption {
      type = with lib.types; attrsOf (submodule ({ name, ... }: {
        options = {
          enable = lib.mkEnableOption "the assertion";

          name = lib.mkOption {
            type = lib.types.str;
            default = name;
          };

          script = lib.mkOption {
            type = lib.types.str;
          };
        };
      }));
      default = { };
    };
  };

  config = {
    # Avoid some standard NixOS assertions
    boot = {
      loader.grub.enable = false;
      initrd.enable = false;
    };
    system.stateVersion = lib.mkDefault lib.trivial.release;

    assertions =
      lib.flip map config.system-manager.etcFiles
        (entry:
          {
            assertion = lib.hasAttr entry config.environment.etc;
            message = lib.concatStringsSep " " [
              "The entry ${entry} that was passed to system-manager.etcFiles"
              "is not present in environment.etc"
            ];
          }
        ) ++
      lib.flip map config.system-manager.services
        (entry:
          {
            assertion = lib.hasAttr entry config.systemd.services;
            message = lib.concatStringsSep " " [
              "The entry ${entry} that was passed to system-manager.services"
              "is not present in systemd.services"
            ];
          }
        );

    system-manager.preActivationAssertions = {
      osVersion =
        let
          supportedIds = [ "nixos" "ubuntu" ];
        in
        {
          enable = !config.system-manager.allowAnyDistro;
          script = ''
            source /etc/os-release
            ${lib.concatStringsSep "\n" (lib.flip map supportedIds (supportedId: ''
              if [ $ID = "${supportedId}" ]; then
                exit 0
              fi
            ''))}
            echo "This OS is not currently supported."
            echo "Supported OSs are: ${lib.concatStringsSep ", " supportedIds}"
            exit 1
          '';
        };
    };

    # Add the system directory for systemd
    system-manager.etcFiles = [ "systemd/system" ];

    environment.etc =
      let
        allowCollisions = false;

        enabledUnits =
          lib.filterAttrs
            (name: _: lib.elem
              name
              (map (name: "${name}.service") config.system-manager.services))
            config.systemd.units;
      in
      {
        "systemd/system".source = lib.mkForce (pkgs.runCommand "system-manager-units"
          {
            preferLocalBuild = true;
            allowSubstitutes = false;
          }
          ''
            mkdir -p $out

            for i in ${toString (lib.mapAttrsToList (n: v: v.unit) enabledUnits)}; do
              fn=$(basename $i/*)
              if [ -e $out/$fn ]; then
                if [ "$(readlink -f $i/$fn)" = /dev/null ]; then
                  ln -sfn /dev/null $out/$fn
                else
                  ${if allowCollisions then ''
                    mkdir -p $out/$fn.d
                    ln -s $i/$fn $out/$fn.d/overrides.conf
                  '' else ''
                    echo "Found multiple derivations configuring $fn!"
                    exit 1
                  ''}
                fi
              else
                ln -fs $i/$fn $out/
              fi
            done

            ${lib.concatStrings (
              lib.mapAttrsToList (name: unit:
                lib.concatMapStrings (name2: ''
                  mkdir -p $out/'${name2}.wants'
                  ln -sfn '../${name}' $out/'${name2}.wants'/
                '') (unit.wantedBy or [])
              ) enabledUnits)}

            ${lib.concatStrings (
              lib.mapAttrsToList (name: unit:
                lib.concatMapStrings (name2: ''
                  mkdir -p $out/'${name2}.requires'
                  ln -sfn '../${name}' $out/'${name2}.requires'/
                '') (unit.requiredBy or [])
              ) enabledUnits)}
          ''
        );
      };
  };
}
