{ config, lib, ... }:
{
  systemd.services.nginx = lib.mkIf config.services.nginx.enable {
    serviceConfig.DynamicUser = true;

    # TODO: can we handle this better?
    wantedBy = lib.mkForce [ "system-manager.target" ];
  };

  # Disable this for now
  services.logrotate.settings.nginx = { };
}
