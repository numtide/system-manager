{
  config,
  pkgs,
  lib,
  utils,
  userborn,
  ...
}:
let
  userbornConfig = {
    groups = lib.mapAttrsToList (username: opts: {
      inherit (opts) name gid members;
    }) config.users.groups;

    users = lib.mapAttrsToList (username: opts: {
      inherit (opts)
        name
        uid
        group
        description
        home
        password
        hashedPassword
        hashedPasswordFile
        initialPassword
        initialHashedPassword
        ;
      isNormal = opts.isNormalUser;
      shell = utils.toShellPath opts.shell;
    }) (lib.filterAttrs (_: u: u.enable) config.users.users);
  };

  previousConfigPath = "/var/lib/userborn/previous-userborn.json";
  userbornConfigJson = pkgs.writeText "userborn.json" (builtins.toJSON userbornConfig);
in
{
  services.userborn.enable = true;
  services.userborn.package = userborn;

  # REMOVE when https://github.com/NixOS/nixpkgs/pull/483684 is merged
  systemd.services.userborn = {
    environment = {
      USERBORN_MUTABLE_USERS = "true";
      USERBORN_PREVIOUS_CONFIG = previousConfigPath;
    };
    serviceConfig = {
      StateDirectory = "userborn";
      ExecStartPost = [
        "${pkgs.coreutils}/bin/ln -sf ${userbornConfigJson} ${previousConfigPath}"
      ];
    };
  };
}
