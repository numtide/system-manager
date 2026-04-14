{
  lib,
  config,
  pkgs,
  ...
}:

{
  options.environment = {
    systemPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
    };

    corePackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
      description = ''
        Packages that are considered essential for the system to function.
        These are automatically included in `environment.systemPackages`.
        NixOS modules use this to register packages they depend on.
      '';
    };

    pathsToLink = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
    };

    extraInit = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "Shell script code which should be called before any shell session through the host /etc/profile.";
    };
  };

  config =
    let
      pathDir = "/run/system-manager/sw";
    in
    {
      environment = {
        systemPackages = config.environment.corePackages;

        pathsToLink = [
          "/bin"
        ];

        etc = {
          "profile.d/system-manager-path.sh".source = pkgs.writeText "system-manager-path.sh" ''
            export PATH=${pathDir}/bin:''${PATH}
            ${config.environment.extraInit}
          '';

          # TODO: figure out how to properly add fish support. We could start by
          # looking at what NixOS and HM do to set up the fish env.
          #"fish/conf.d/system-manager-path.fish".source =
          #  pkgs.writeTextFile {
          #    name = "system-manager-path.fish";
          #    executable = true;
          #    text = ''
          #      set -gx PATH "${pathDir}/bin/" $PATH
          #    '';
          #  };
        };
      };

      systemd.services.system-manager-path = {
        enable = true;
        description = "";
        wantedBy = [ "system-manager.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script =
          let
            pathDrv = pkgs.buildEnv {
              name = "system-manager-path";
              paths = config.environment.systemPackages;
              inherit (config.environment) pathsToLink;
            };
          in
          ''
            mkdir --parents $(dirname "${pathDir}")
            if [ -L "${pathDir}" ]; then
              unlink "${pathDir}"
            fi
            ln --symbolic --force "${pathDrv}" "${pathDir}"
          '';
      };
    };
}
