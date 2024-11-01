{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    services.nginx.enable = true;

    environment = {
      systemPackages = [
        pkgs.ripgrep
        pkgs.fd
      ];

      etc = {
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
    };

    systemd.services = lib.listToAttrs (
      lib.flip lib.genList 10 (
        ix:
        lib.nameValuePair "service-${toString ix}" {
          enable = true;
          description = "service-${toString ix}";
          wants = [ "network-online.target" ];
          after = [
            "network-online.target"
          ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
          };
          wantedBy = [ "system-manager.target" ];
          requiredBy = lib.mkIf (ix > 5) [ "service-0.service" ];
          script = ''
            sleep ${if ix > 5 then "2" else "1"}
          '';
        }
      )
    );
    systemd.tmpfiles.rules = [ "D /var/tmp/system-manager 0755 root root -" ];
    systemd.tmpfiles.settings.sample = {
      "/var/tmp/sample".d = {
        mode = "0755";
      };
    };
  };
}
