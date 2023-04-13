{
  systemd.services.nginx.serviceConfig.DynamicUser = true;

  # Disable this for now
  services.logrotate.settings.nginx = { };
}
