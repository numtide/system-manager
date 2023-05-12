{ lib
, system-manager
, system
}:

let
  forEachUbuntuImage = lib.flip lib.mapAttrs' system-manager.lib.images.ubuntu.${system};

  newConfig = system-manager.lib.makeSystemConfig {
    modules = [
      ({ lib, pkgs, ... }: {
        config = {
          nixpkgs.hostPlatform = system;

          services.nginx.enable = true;

          environment.etc = {
            foo_new = {
              text = ''
                This is just a test!
              '';
            };
          };

          systemd.services.new-service = {
            enable = true;
            description = "new-service";
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
              ExecReload = "${lib.getBin pkgs.coreutils}/bin/true";
            };
            wantedBy = [ "system-manager.target" "default.target" ];
            script = ''
              sleep 2
            '';
          };
        };
      })
    ];
  };
in

forEachUbuntuImage
  (imgName: image: lib.nameValuePair
    "vm-test-example-${imgName}"
    (system-manager.lib.make-vm-test "vm-test-example-${imgName}" {
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
                  ../../../examples/example.nix
                ];

                virtualisation.rootImage = system-manager.lib.prepareUbuntuImage {
                  inherit hostPkgs image;
                  nodeConfig = config;
                };
              };
            };

            testScript = ''
              # Start all machines in parallel
              start_all()

              node1.wait_for_unit("default.target")

              node1.succeed("/system-manager-profile/bin/activate")
              node1.wait_for_unit("system-manager.target")

              node1.wait_for_unit("service-9.service")
              node1.wait_for_file("/etc/baz/bar/foo2")
              node1.wait_for_file("/etc/a/nested/example/foo3")
              node1.wait_for_file("/etc/foo.conf")
              node1.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
              node1.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

              node1.succeed("${newConfig}/bin/activate")
              node1.wait_for_unit("new-service.service")
              node1.wait_until_fails("systemctl status service-9.service")
              node1.wait_until_fails("cat /etc/a/nested/example/foo3")
              node1.wait_until_fails("cat /etc/baz/bar/foo2")
              node1.wait_for_file("/etc/foo_new")

              # Simulate a reboot, to check that the services defined with
              # system-manager start correctly after a reboot.
              # TODO: can we find an easy way to really reboot the VM and not
              # loose the root FS state?
              node1.systemctl("isolate rescue.target")
              # We need to send a return character to dismiss the rescue-mode prompt
              node1.send_key("ret")
              node1.systemctl("isolate default.target")
              node1.wait_for_unit("default.target")

              node1.wait_for_unit("new-service.service")
              node1.wait_until_fails("systemctl status service-9.service")
              node1.wait_until_fails("cat /etc/a/nested/example/foo3")
              node1.wait_until_fails("cat /etc/baz/bar/foo2")
              node1.wait_for_file("/etc/foo_new")

              node1.succeed("${newConfig}/bin/deactivate")
              node1.wait_until_fails("systemctl status new-service.service")
              node1.wait_until_fails("cat /etc/foo_new")
            '';
          })
      ];
    })
  )

  //

forEachUbuntuImage
  (imgName: image: lib.nameValuePair
    "vm-test-prepopulate-${imgName}"
    (system-manager.lib.make-vm-test "vm-test-prepopulate-${imgName}" {
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
                  ../../../examples/example.nix
                ];

                virtualisation.rootImage = system-manager.lib.prepareUbuntuImage {
                  inherit hostPkgs image;
                  nodeConfig = config;
                };
              };
            };

            testScript = ''
              # Start all machines in parallel
              start_all()

              node1.wait_for_unit("default.target")

              node1.succeed("/system-manager-profile/bin/prepopulate")
              node1.systemctl("daemon-reload")
              node1.systemctl("start default.target")
              node1.wait_for_unit("system-manager.target")

              node1.wait_for_unit("service-9.service")
              node1.wait_for_file("/etc/baz/bar/foo2")
              node1.wait_for_file("/etc/a/nested/example/foo3")
              node1.wait_for_file("/etc/foo.conf")
              node1.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
              node1.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

              node1.succeed("${newConfig}/bin/activate")
              node1.wait_for_unit("new-service.service")
              node1.wait_until_fails("systemctl status service-9.service")
              node1.wait_until_fails("cat /etc/a/nested/example/foo3")
              node1.wait_until_fails("cat /etc/baz/bar/foo2")
              node1.wait_for_file("/etc/foo_new")

              node1.succeed("${newConfig}/bin/deactivate")
              node1.wait_until_fails("systemctl status new-service.service")
              node1.wait_until_fails("cat /etc/foo_new")
            '';
          }
        )
      ];
    })
  )
