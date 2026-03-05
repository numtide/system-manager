{ config, lib, ... }:
{
  # Stub for option referenced by upstream wrappers module that system-manager lacks
  options.security.apparmor.includes = lib.mkOption {
    type = lib.types.attrsOf lib.types.lines;
    default = { };
    internal = true;
  };

  config = lib.mkIf config.security.enableWrappers {
    systemd.services.suid-sgid-wrappers = {
      wantedBy = lib.mkForce [ "system-manager.target" ];
      before = lib.mkForce [ "system-manager.target" ];
      after = lib.mkForce [
        "userborn.service"
        "run-wrappers.mount"
      ];
    };
  };
}
