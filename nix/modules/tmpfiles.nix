{
  config,
  lib,
  pkgs,
  ...
}:
let
  inherit (lib)
    concatStrings
    concatStringsSep
    getLib
    literalExpression
    mapAttrsToList
    mkOption
    types
    ;
  # copied from nixos/modules/system/boot/systemd/tmpfiles.nix
  settingsOption = {
    description = ''
      Declare systemd-tmpfiles rules to create, delete, and clean up volatile
      and temporary files and directories.

      Even though the service is called `*tmp*files` you can also create
      persistent files.
    '';
    example = {
      "10-mypackage" = {
        "/var/lib/my-service/statefolder".d = {
          mode = "0755";
          user = "root";
          group = "root";
        };
      };
    };
    default = { };
    type = types.attrsOf (
      types.attrsOf (
        types.attrsOf (
          types.submodule (
            { name, ... }:
            {
              options = {
                type = mkOption {
                  type = types.str;
                  default = name;
                  example = "d";
                  description = ''
                    The type of operation to perform on the file.

                    The type consists of a single letter and optionally one or more
                    modifier characters.

                    Please see the upstream documentation for the available types and
                    more details:
                    <https://www.freedesktop.org/software/systemd/man/tmpfiles.d>
                  '';
                };
                mode = mkOption {
                  type = types.str;
                  default = "-";
                  example = "0755";
                  description = ''
                    The file access mode to use when creating this file or directory.
                  '';
                };
                user = mkOption {
                  type = types.str;
                  default = "-";
                  example = "root";
                  description = ''
                    The user of the file.

                    This may either be a numeric ID or a user/group name.

                    If omitted or when set to `"-"`, the user and group of the user who
                    invokes systemd-tmpfiles is used.
                  '';
                };
                group = mkOption {
                  type = types.str;
                  default = "-";
                  example = "root";
                  description = ''
                    The group of the file.

                    This may either be a numeric ID or a user/group name.

                    If omitted or when set to `"-"`, the user and group of the user who
                    invokes systemd-tmpfiles is used.
                  '';
                };
                age = mkOption {
                  type = types.str;
                  default = "-";
                  example = "10d";
                  description = ''
                    Delete a file when it reaches a certain age.

                    If a file or directory is older than the current time minus the age
                    field, it is deleted.

                    If set to `"-"` no automatic clean-up is done.
                  '';
                };
                argument = mkOption {
                  type = types.str;
                  default = "";
                  example = "";
                  description = ''
                    An argument whose meaning depends on the type of operation.

                    Please see the upstream documentation for the meaning of this
                    parameter in different situations:
                    <https://www.freedesktop.org/software/systemd/man/tmpfiles.d>
                  '';
                };
              };
            }
          )
        )
      )
    );
  };

  # generates a single entry for a tmpfiles.d rule
  settingsEntryToRule = path: entry: ''
    '${entry.type}' '${path}' '${entry.mode}' '${entry.user}' '${entry.group}' '${entry.age}' ${entry.argument}
  '';

  # generates a list of tmpfiles.d rules from the attrs (paths) under tmpfiles.settings.<name>
  pathsToRules = mapAttrsToList (
    path: types: concatStrings (mapAttrsToList (_type: settingsEntryToRule path) types)
  );

  mkRuleFileContent = paths: concatStrings (pathsToRules paths);

in
{
  options = {
    systemd.tmpfiles.rules = lib.mkOption {
      type = types.listOf types.str;
      default = [ ];
      example = [ "d /tmp 1777 root root 10d" ];
      description = lib.mdDoc ''
        Rules for creation, deletion and cleaning of volatile and temporary files
        automatically. See
        {manpage}`tmpfiles.d(5)`
        for the exact format.
      '';
    };

    systemd.tmpfiles.settings = lib.mkOption settingsOption;

    systemd.tmpfiles.packages = mkOption {
      type = types.listOf types.package;
      default = [ ];
      example = literalExpression "[ pkgs.lvm2 ]";
      apply = map getLib;
      description = ''
        List of packages containing {command}`systemd-tmpfiles` rules.

        All files ending in .conf found in
        {file}`«pkg»/lib/tmpfiles.d`
        will be included.
        If this folder does not exist or does not contain any files an error will be returned instead.

        If a {file}`lib` output is available, rules are searched there and only there.
        If there is no {file}`lib` output it will fall back to {file}`out`
        and if that does not exist either, the default output will be used.
      '';
    };

  };

  config = {
    environment.etc = {
      "tmpfiles.d".source = pkgs.symlinkJoin {
        name = "tmpfiles.d";
        paths = map (p: p + "/lib/tmpfiles.d") config.systemd.tmpfiles.packages;
        postBuild = ''
          for i in $(cat $pathsPath); do
            (test -d "$i" && test $(ls "$i"/*.conf | wc -l) -ge 1) || (
              echo "ERROR: The path '$i' from systemd.tmpfiles.packages contains no *.conf files."
              exit 1
            )
          done
        '';
      };
    };
    systemd.tmpfiles.packages = [
      (pkgs.writeTextFile {
        name = "system-manager-tmpfiles.d";
        destination = "/lib/tmpfiles.d/00-system-manager.conf";
        text = ''
          # This file is created automatically and should not be modified.
          # Please change the option ‘systemd.tmpfiles.rules’ instead.

          ${concatStringsSep "\n" config.systemd.tmpfiles.rules}
        '';
      })
    ]
    ++ (mapAttrsToList (
      name: paths: pkgs.writeTextDir "lib/tmpfiles.d/${name}.conf" (mkRuleFileContent paths)
    ) config.systemd.tmpfiles.settings);
  };
}
