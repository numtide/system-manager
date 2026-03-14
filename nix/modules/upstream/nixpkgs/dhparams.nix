{ config, lib, ... }:
let
  cfg = config.security.dhparams;
in
{
  config = lib.mkIf (cfg.enable && cfg.stateful) {
    systemd.services = {
      dhparams-init.wantedBy = lib.mkForce [ "system-manager.target" ];
    }
    // lib.mapAttrs' (
      name: _:
      lib.nameValuePair "dhparams-gen-${name}" {
        wantedBy = lib.mkForce [ "system-manager.target" ];
      }
    ) cfg.params;
  };
}
