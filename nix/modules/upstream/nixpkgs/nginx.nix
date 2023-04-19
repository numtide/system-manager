{ lib, ... }:
{
  systemd.services.nginx = {
    serviceConfig.DynamicUser = true;

    # TODO: can we handle this better?
    wantedBy = lib.mkForce [
      "system-manager.target"
    ];
  };

  # Disable this for now
  services.logrotate.settings.nginx = { };
}
