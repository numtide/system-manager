{
  lib,
  system-manager,
  system,
  nix-vm-test,
}:

let
  forEachUbuntuImage =
    name:
    {
      modules,
      testScriptFunction,
      extraPathsToRegister ? [ ],
      projectTest ? test: test.sandboxed,
    }:
    let
      ubuntu = nix-vm-test.ubuntu;
    in
    lib.listToAttrs (
      lib.flip map (lib.attrNames ubuntu.images) (
        imageVersion:
        let
          toplevel = (
            system-manager.lib.makeSystemConfig {
              modules = modules ++ [
                (
                  { lib, pkgs, ... }:
                  {
                    options.hostPkgs = lib.mkOption {
                      type = lib.types.raw;
                      readOnly = true;
                    };
                    config.hostPkgs = pkgs;
                  }
                )
              ];
            }
          );
          inherit (toplevel.config) hostPkgs;
        in
        lib.nameValuePair "ubuntu-${imageVersion}-${name}" (
          projectTest (
            ubuntu.${imageVersion} {
              testScript = testScriptFunction { inherit toplevel hostPkgs; };
              extraPathsToRegister = extraPathsToRegister ++ [
                toplevel
              ];
              sharedDirs = { };
            }
          )
        )
      )
    );

  # To test reload and restart, we include two services, one that can be reloaded
  # and one that cannot.
  # The id parameter is a string that can be used to force reloading the services
  # between two configs by changing their contents.
  testModule =
    id:
    { lib, pkgs, ... }:
    {
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
      (
        { lib, pkgs, ... }:
        {
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
                wantedBy = [
                  "system-manager.target"
                  "default.target"
                ];
                script = ''
                  sleep 2
                '';
              };
            };
          };
        }
      )
    ];
  };

in

forEachUbuntuImage "example" {
  modules = [
    (testModule "old")
    ../../../examples/example.nix
  ];
  extraPathsToRegister = [ newConfig ];
  testScriptFunction =
    { toplevel, ... }:
    ''
      # Start all machines in parallel
      start_all()

      vm.wait_for_unit("default.target")

      vm.succeed("touch /etc/foo_test")
      vm.succeed("${toplevel}/bin/activate 2>&1 | tee /tmp/output.log")
      vm.succeed("grep -F 'Error while creating file in /etc: Unmanaged path already exists in filesystem, please remove it and run system-manager again: /etc/foo_test' /tmp/output.log")
      vm.succeed("rm /etc/foo_test")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}
      vm.wait_for_unit("system-manager.target")

      vm.succeed("systemctl status service-9.service")
      vm.succeed("test -f /etc/baz/bar/foo2")
      vm.succeed("test -f /etc/a/nested/example/foo3")
      vm.succeed("test -f /etc/foo.conf")
      vm.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
      vm.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

      vm.succeed("test -d /var/tmp/system-manager")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.succeed("systemctl status new-service.service")
      vm.fail("systemctl status service-9.service")
      vm.fail("test -f /etc/a/nested/example/foo3")
      vm.fail("test -f /etc/baz/bar/foo2")
      vm.fail("test -f /etc/systemd/system/nginx.service")
      vm.succeed("test -f /etc/foo_new")

      vm.succeed("test -d /var/tmp/system-manager")
      vm.succeed("touch /var/tmp/system-manager/foo1")

      # Simulate a reboot, to check that the services defined with
      # system-manager start correctly after a reboot.
      # TODO: can we find an easy way to really reboot the VM and not
      # loose the root FS state?
      vm.systemctl("isolate rescue.target")
      # We need to send a return character to dismiss the rescue-mode prompt
      vm.send_key("ret")
      vm.systemctl("isolate default.target")
      vm.wait_for_unit("default.target")

      vm.succeed("systemctl status new-service.service")
      vm.fail("systemctl status service-9.service")
      vm.fail("test -f /etc/a/nested/example/foo3")
      vm.fail("test -f /etc/baz/bar/foo2")
      vm.succeed("test -f /etc/foo_new")

      ${system-manager.lib.deactivateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.fail("systemctl status new-service.service")
      vm.fail("test -f /etc/foo_new")
      #vm.fail("test -f /var/tmp/system-manager/foo1")
    '';
}

//

  forEachUbuntuImage "prepopulate" {
    modules = [
      (testModule "old")
      ../../../examples/example.nix
    ];
    extraPathsToRegister = [ newConfig ];
    testScriptFunction =
      { toplevel, ... }:
      ''
        # Start all machines in parallel
        start_all()

        vm.wait_for_unit("default.target")

        ${system-manager.lib.prepopulateProfileSnippet {
          node = "vm";
          profile = toplevel;
        }}
        vm.systemctl("daemon-reload")

        # Simulate a reboot, to check that the services defined with
        # system-manager start correctly after a reboot.
        # TODO: can we find an easy way to really reboot the VM and not
        # loose the root FS state?
        vm.systemctl("isolate rescue.target")
        # We need to send a return character to dismiss the rescue-mode prompt
        vm.send_key("ret")
        vm.systemctl("isolate default.target")
        vm.wait_for_unit("system-manager.target")

        vm.succeed("systemctl status service-9.service")
        vm.succeed("test -f /etc/baz/bar/foo2")
        vm.succeed("test -f /etc/a/nested/example/foo3")
        vm.succeed("test -f /etc/foo.conf")
        vm.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
        vm.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

        ${system-manager.lib.activateProfileSnippet {
          node = "vm";
          profile = newConfig;
        }}
        vm.succeed("systemctl status new-service.service")
        vm.fail("systemctl status service-9.service")
        vm.fail("test -f /etc/a/nested/example/foo3")
        vm.fail("test -f /etc/baz/bar/foo2")
        vm.succeed("test -f /etc/foo_new")

        ${system-manager.lib.deactivateProfileSnippet {
          node = "vm";
          profile = newConfig;
        }}
        vm.fail("systemctl status new-service.service")
        vm.fail("test -f /etc/foo_new")
      '';
  }

//

  forEachUbuntuImage "system-path" {
    modules = [
      (testModule "old")
      ../../../examples/example.nix
    ];
    extraPathsToRegister = [ newConfig ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        # Start all machines in parallel
        start_all()
        vm.wait_for_unit("default.target")

        vm.fail("bash --login -c '$(which rg)'")
        vm.fail("bash --login -c '$(which fd)'")

        ${system-manager.lib.activateProfileSnippet {
          node = "vm";
          profile = toplevel;
        }}

        vm.wait_for_unit("system-manager.target")
        vm.wait_for_unit("system-manager-path.service")

        #vm.fail("bash --login -c '$(which fish)'")
        vm.succeed("bash --login -c 'realpath $(which rg) | grep -F ${hostPkgs.ripgrep}/bin/rg'")
        vm.succeed("bash --login -c 'realpath $(which fd) | grep -F ${hostPkgs.fd}/bin/fd'")

        ${system-manager.lib.activateProfileSnippet {
          node = "vm";
          profile = newConfig;
        }}

        vm.fail("bash --login -c '$(which rg)'")
        vm.fail("bash --login -c '$(which fd)'")
        vm.succeed("bash --login -c 'realpath $(which fish) | grep -F ${hostPkgs.fish}/bin/fish'")
      '';
  }
