{ pkgs, ... }: {
  systemd.services =
    let
      service-1 = "service-1";
      service-2 = "service-2";
    in
    {
      ${service-1} = {
        enable = true;
        description = service-1;
        wants = [ "network-online.target" ];
        after = [
          "network-online.target"
          "avahi-daemon.service"
          "chrony.service"
          "nss-lookup.target"
          "tinc.service"
          "pulseaudio.service"
        ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          script = ''
            true
          '';
          ExecReload = "true";
        };
        wantedBy = [ "multi-user.target" ];
      };

      ${service-2} = {
        enable = true;
        description = service-2;
        serviceConfig = {
          Type = "simple";
        };
        partOf = [ "${service-1}.service" ];
        wantedBy = [ "${service-1}.service" ];

        script = ''
          true
        '';
      };
    };
}
