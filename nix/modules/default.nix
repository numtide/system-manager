{ lib
, pkgs
, ...
}:
let
  services =
    lib.listToAttrs
      (lib.flip lib.genList 10 (ix:
        lib.nameValuePair "service-${toString ix}"
          {
            enable = true;
            description = "service-${toString ix}";
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
              ExecReload = "true";
            };
            wantedBy = [ "multi-user.target" ];
            script = ''
              sleep ${if ix > 5 then "3" else "1"}
            '';
          })
      );
in
{
  options = {
    system-manager.services = lib.mkOption {
      type = with lib.types; listOf str;
    };
  };

  config = {
    system-manager.services = lib.attrNames services;
    systemd = { inherit services; };
  };
}
