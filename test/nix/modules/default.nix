{ system-manager
, system
}:

let
  testConfig = { lib, pkgs, ... }: {
    config = {
      nixpkgs.hostPlatform = "x86_64-linux";

      services.nginx.enable = true;

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
                ];
                serviceConfig = {
                  Type = "oneshot";
                  RemainAfterExit = true;
                  ExecReload = "${lib.getBin pkgs.coreutils}/bin/true";
                };
                wantedBy = [ "system-manager.target" ];
                requiredBy = lib.mkIf (ix > 5) [ "service-0.service" ];
                script = ''
                  sleep ${if ix > 5 then "2" else "1"}
                '';
              })
          );
    };
  };
in
system-manager.lib.make-vm-test {
  inherit system;
  modules = [
    ({ config, ... }:
      let
        inherit (config) hostPkgs;
      in
      {
        nodes = {
          node1 = { config, ... }: {
            modules = [
              testConfig
            ];

            virtualisation.rootImage = system-manager.lib.prepareUbuntuImage {
              inherit hostPkgs;
              nodeConfig = config;
              image = system-manager.lib.images.ubuntu_22_10_cloudimg;
            };
          };
        };

        testScript = ''
          # Start all machines in parallel
          start_all()

          node1.wait_for_unit("default.target")

          node1.execute("/system-manager-profile/bin/activate")
          node1.wait_for_unit("system-manager.target")

          node1.wait_for_unit("service-9.service")
          node1.wait_for_file("/etc/baz/bar/foo2")
          node1.wait_for_file("/etc/foo.conf")
          node1.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
          node1.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")
        '';
      })
  ];
}
