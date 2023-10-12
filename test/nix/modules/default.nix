{ lib
, system-manager
, system
}:

let
  images = lib.importJSON ../images.json;
  forEachUbuntuImage = lib.flip lib.mapAttrs' images.ubuntu.${system};

  # To test reload and restart, we include two services, one that can be reloaded
  # and one that cannot.
  # The id parameter is a string that can be used to force reloading the services
  # between two configs by changing their contents.
  testModule = id: { lib, pkgs, ... }: {
    systemd.services = {
      has-reload = {
        enable = true;
        description = "service-reload";
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecReload = ''
            ${lib.getBin pkgs.coreutils}/bin/true
          '';
        };
        wantedBy = [ "system-manager.target" ];
        script = ''
          echo "I can be reloaded (id: ${id})"
        '';
      };
      has-no-reload = {
        enable = true;
        description = "service-no-reload";
        serviceConfig.Type = "simple";
        wantedBy = [ "system-manager.target" ];
        script = ''
          while true; do
            echo "I cannot be reloaded (id: ${id})"
          done
        '';
      };
    };
  };

  newConfig = system-manager.lib.makeSystemConfig {
    modules = [
      (testModule "new")
      ({ lib, pkgs, ... }: {
        config = {
          nixpkgs.hostPlatform = system;

          services.nginx.enable = false;

          environment = {
            etc = {
              foo_new = {
                text = ''
                  This is just a test!
                '';
              };
            };

            systemPackages = [
              pkgs.fish
            ];
          };

          systemd.services = {
            new-service = {
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
                  (testModule "old")
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

              node1.succeed("touch /etc/foo_test")
              node1.succeed("/system-manager-profile/bin/activate 2>&1 | tee /tmp/output.log")
              node1.succeed("grep -F 'Error while creating file in /etc: Unmanaged path already exists in filesystem, please remove it and run system-manager again: /etc/foo_test' /tmp/output.log")
              node1.succeed("rm /etc/foo_test")

              ${system-manager.lib.activateProfileSnippet { node = "node1"; }}
              node1.wait_for_unit("system-manager.target")

              node1.succeed("systemctl status service-9.service")
              node1.succeed("cat /etc/baz/bar/foo2")
              node1.succeed("cat /etc/a/nested/example/foo3")
              node1.succeed("cat /etc/foo.conf")
              node1.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
              node1.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

              node1.succeed("test -d /var/tmp/system-manager")

              ${system-manager.lib.activateProfileSnippet { node = "node1"; profile = newConfig; }}
              node1.succeed("systemctl status new-service.service")
              node1.fail("systemctl status service-9.service")
              node1.fail("cat /etc/a/nested/example/foo3")
              node1.fail("cat /etc/baz/bar/foo2")
              node1.fail("cat /etc/systemd/system/nginx.service")
              node1.succeed("cat /etc/foo_new")

              node1.succeed("test -d /var/tmp/system-manager")
              node1.succeed("touch /var/tmp/system-manager/foo1")

              # Simulate a reboot, to check that the services defined with
              # system-manager start correctly after a reboot.
              # TODO: can we find an easy way to really reboot the VM and not
              # loose the root FS state?
              node1.systemctl("isolate rescue.target")
              # We need to send a return character to dismiss the rescue-mode prompt
              node1.send_key("ret")
              node1.systemctl("isolate default.target")
              node1.wait_for_unit("default.target")

              node1.succeed("systemctl status new-service.service")
              node1.fail("systemctl status service-9.service")
              node1.fail("cat /etc/a/nested/example/foo3")
              node1.fail("cat /etc/baz/bar/foo2")
              node1.succeed("cat /etc/foo_new")

              ${system-manager.lib.deactivateProfileSnippet { node = "node1"; profile = newConfig; }}
              node1.fail("systemctl status new-service.service")
              node1.fail("cat /etc/foo_new")
              #node1.fail("test -f /var/tmp/system-manager/foo1")
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

              ${system-manager.lib.prepopulateProfileSnippet { node = "node1"; }}
              node1.systemctl("daemon-reload")

              # Simulate a reboot, to check that the services defined with
              # system-manager start correctly after a reboot.
              # TODO: can we find an easy way to really reboot the VM and not
              # loose the root FS state?
              node1.systemctl("isolate rescue.target")
              # We need to send a return character to dismiss the rescue-mode prompt
              node1.send_key("ret")
              node1.systemctl("isolate default.target")
              node1.wait_for_unit("system-manager.target")

              node1.succeed("systemctl status service-9.service")
              node1.succeed("cat /etc/baz/bar/foo2")
              node1.succeed("cat /etc/a/nested/example/foo3")
              node1.succeed("cat /etc/foo.conf")
              node1.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
              node1.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

              ${system-manager.lib.activateProfileSnippet { node = "node1"; profile = newConfig; }}
              node1.succeed("systemctl status new-service.service")
              node1.fail("systemctl status service-9.service")
              node1.fail("cat /etc/a/nested/example/foo3")
              node1.fail("cat /etc/baz/bar/foo2")
              node1.succeed("cat /etc/foo_new")

              ${system-manager.lib.deactivateProfileSnippet { node = "node1"; profile = newConfig; }}
              node1.fail("systemctl status new-service.service")
              node1.fail("cat /etc/foo_new")
            '';
          }
        )
      ];
    })
  )

  //

forEachUbuntuImage
  (imgName: image: lib.nameValuePair
    "vm-test-system-path-${imgName}"
    (system-manager.lib.make-vm-test "vm-test-system-path-${imgName}" {
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

              node1.fail("bash --login -c '$(which rg)'")
              node1.fail("bash --login -c '$(which fd)'")

              ${system-manager.lib.activateProfileSnippet { node = "node1"; }}

              node1.wait_for_unit("system-manager.target")
              node1.wait_for_unit("system-manager-path.service")

              node1.fail("bash --login -c '$(which fish)'")
              node1.succeed("bash --login -c 'realpath $(which rg) | grep -F ${hostPkgs.ripgrep}/bin/rg'")
              node1.succeed("bash --login -c 'realpath $(which fd) | grep -F ${hostPkgs.fd}/bin/fd'")

              ${system-manager.lib.activateProfileSnippet { node = "node1"; profile = newConfig; }}

              node1.fail("bash --login -c '$(which rg)'")
              node1.fail("bash --login -c '$(which fd)'")
              node1.succeed("bash --login -c 'realpath $(which fish) | grep -F ${hostPkgs.fish}/bin/fish'")
            '';
          })
      ];
    })
  )
