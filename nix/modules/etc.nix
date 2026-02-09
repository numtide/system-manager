{
  lib,
  pkgs,
  ...
}:
{
  options = {
    system.etc = {
      overlay = {
        enable = lib.mkEnableOption "systemd-sysusers" // {
          description = ''
            If enabled, users are created with systemd-sysusers instead of with
            the custom `update-users-groups.pl` script.

            Note: This is experimental.
          '';
        };
      };
    };

    environment.etc = lib.mkOption {
      default = { };
      example = lib.literalExpression ''
        { example-configuration-file =
            { source = "/nix/store/.../etc/dir/file.conf.example";
              mode = "0440";
            };
          "default/useradd".text = "GROUP=100 ...";
        }
      '';
      description = lib.mdDoc ''
        Set of files that have to be linked in {file}`/etc`.
      '';

      type = lib.types.attrsOf (
        lib.types.submodule (
          {
            name,
            config,
            options,
            ...
          }:
          {
            options = {

              enable = lib.mkOption {
                type = lib.types.bool;
                default = true;
                description = lib.mdDoc ''
                  Whether this /etc file should be generated.  This
                  option allows specific /etc files to be disabled.
                '';
              };

              target = lib.mkOption {
                type = lib.types.str;
                description = lib.mdDoc ''
                  Name of symlink (relative to
                  {file}`/etc`).  Defaults to the attribute
                  name.
                '';
              };

              text = lib.mkOption {
                default = null;
                type = lib.types.nullOr lib.types.lines;
                description = lib.mdDoc "Text of the file.";
              };

              source = lib.mkOption {
                type = lib.types.path;
                description = lib.mdDoc "Path of the source file.";
              };

              mode = lib.mkOption {
                type = lib.types.str;
                default = "symlink";
                example = "0600";
                description = lib.mdDoc ''
                  If set to something else than `symlink`,
                  the file is copied instead of symlinked, with the given
                  file mode.
                '';
              };

              uid = lib.mkOption {
                default = 0;
                type = lib.types.int;
                description = lib.mdDoc ''
                  UID of created file. Only takes effect when the file is
                  copied (that is, the mode is not 'symlink').
                '';
              };

              gid = lib.mkOption {
                default = 0;
                type = lib.types.int;
                description = lib.mdDoc ''
                  GID of created file. Only takes effect when the file is
                  copied (that is, the mode is not 'symlink').
                '';
              };

              user = lib.mkOption {
                default = "+${toString config.uid}";
                type = lib.types.str;
                description = lib.mdDoc ''
                  User name of created file.
                  Only takes effect when the file is copied (that is, the mode is not 'symlink').
                  Changing this option takes precedence over `uid`.
                '';
              };

              group = lib.mkOption {
                default = "+${toString config.gid}";
                type = lib.types.str;
                description = lib.mdDoc ''
                  Group name of created file.
                  Only takes effect when the file is copied (that is, the mode is not 'symlink').
                  Changing this option takes precedence over `gid`.
                '';
              };

              replaceExisting = lib.mkOption {
                type = lib.types.bool;
                default = false;
                description = lib.mdDoc ''
                  Whether to replace a pre-existing file at the target path.
                  When enabled, the existing file is backed up to
                  `{file}`<path>.system-manager-backup` before being replaced.
                  The backup is restored when system-manager is deactivated or
                  when the entry is removed from the configuration.
                '';
              };
            };

            config = {
              target = lib.mkDefault name;
              source = lib.mkIf (config.text != null) (
                let
                  name' = "etc-" + baseNameOf name;
                in
                lib.mkDerivedConfig options.text (pkgs.writeText name')
              );
            };
          }
        )
      );
    };
  };
  config = {
    system.etc.overlay.enable = false;
  };
}
