{ lib
, pkgs
, ...
}:
{
  config = {
    system-manager = {
      environment.etc = {
        foo = {
          text = ''
            This is just a test!
          '';
          target = "foo_test";
        };

        "foo.conf".text = ''
          launch_the_rockets = true
        '';

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
      systemd.services =
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
                  ExecReload = "${lib.getBin pkgs.coreutils}/bin/true";
                };
                wantedBy = [ "multi-user.target" ];
                requiredBy = lib.mkIf (ix > 5) [ "service-0.service" ];
                script = ''
                  sleep ${if ix > 5 then "2" else "1"}
                '';
              })
          );
    };
  };
}
