{ lib
, pkgs
, config
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

  etcFiles = {
    foo = {
      text = ''
        This is just a test!
      '';
      target = "foo_test";
    };

    "baz/bar/foo2" = {
      text = ''
        Another test!
      '';
      mode = "symlink";
    };

    foo3 = {
      text = "boo!";
      mode = "0700";
      user = "root";
      group = "root";
    };

    "a/nested/example/foo3" = {
      text = "boo!";
      mode = "0764";
      user = "root";
      group = "root";
    };

    "a/nested/example2/foo3" = {
      text = "boo!";
      mode = "0764";
      user = "root";
      group = "root";
    };

    out-of-store = {
      source = "/run/systemd/system/";
    };
  };
in
{
  config = {
    system-manager = {
      etcFiles = lib.attrNames etcFiles;
      services = lib.attrNames services;
    };
    environment.etc = etcFiles;
    systemd = { inherit services; };
  };
}
