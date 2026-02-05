{ config, lib, ... }:
{
  config = lib.mkIf config.services.nginx.enable {
    users.users.nginx.uid = lib.mkForce 980;
    users.groups.nginx.gid = lib.mkForce 980;

    systemd.services.nginx = {
      serviceConfig.DynamicUser = true;

      # TODO: can we handle this better?
      wantedBy = lib.mkForce [
        "system-manager.target"
      ];
    };

    # Disable this for now
    services.logrotate.settings.nginx = { };
  };
}
