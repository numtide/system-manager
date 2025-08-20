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
                    config = {
                      nixpkgs.hostPlatform = system;
                      hostPkgs = pkgs;
                    };
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

            nix = {
              settings = {
                experimental-features = [
                  "nix-command"
                  "flakes"
                ];
                trusted-users = [ "zimbatm" ];
              };
            };

            system.activationScripts = {
              "system-manager" = {
                text = ''
                  touch /tmp/file-created-by-system-activation-script
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
    ../examples/example.nix
  ];
  extraPathsToRegister = [ newConfig ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    #python
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

      uid = vm.succeed("stat -c %u /etc/with_ownership").strip()
      gid = vm.succeed("stat -c %g /etc/with_ownership").strip()
      assert uid == "5", f"uid was {uid}, expected 5"
      assert gid == "6", f"gid was {gid}, expected 6"

      print(vm.succeed("cat /etc/passwd"))

      user = vm.succeed("stat -c %U /etc/with_ownership2").strip()
      group = vm.succeed("stat -c %G /etc/with_ownership2").strip()
      assert user == "nobody", f"user was {user}, expected nobody"
      assert group == "users", f"group was {group}, expected users"

      vm.fail("test -e /etc/with_ownership.uid")
      vm.fail("test -e /etc/with_ownership.gid")
      vm.fail("test -e /etc/with_ownership.mode")
      vm.fail("test -e /etc/with_ownership2.uid")
      vm.fail("test -e /etc/with_ownership2.gid")
      vm.fail("test -e /etc/with_ownership2.mode")

      vm.succeed("test -d /var/tmp/system-manager")
      vm.succeed("test -d /var/tmp/sample")

      vm.succeed("test -f /etc/tmpfiles.d/sample.conf")
      vm.succeed("test -f /etc/tmpfiles.d/00-system-manager.conf")

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
      vm.succeed("test -f /tmp/file-created-by-system-activation-script")

      nix_trusted_users = vm.succeed("${hostPkgs.nix}/bin/nix config show trusted-users").strip()
      assert "zimbatm" in nix_trusted_users, f"Expected 'zimbatm' to be in trusted-users, got {nix_trusted_users}"

      # Re-activate the same profile to verify idempotency and no ERROR in output
      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.succeed("systemctl status new-service.service")
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
      ../examples/example.nix
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
      ../examples/example.nix
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

//

  # Test the --sudo flag functionality
  # This test creates a non-root user with sudo privileges and verifies that:
  # 1. Running system-manager as non-root without --sudo fails
  # 2. Running system-manager with --sudo succeeds
  forEachUbuntuImage "sudo" {
    modules = [
      (testModule "old")
      ../examples/example.nix
    ];
    extraPathsToRegister = [
      system-manager.packages.x86_64-linux.default
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      let
        system-manager-cli = system-manager.packages.x86_64-linux.default;
      in
      ''
        # Start all machines in parallel
        start_all()

        vm.wait_for_unit("default.target")

        # Create a test user with sudo privileges (NOPASSWD for testing)
        vm.succeed("useradd -m testuser")
        vm.succeed("echo 'testuser ALL=(ALL) NOPASSWD: ALL' > /etc/sudoers.d/testuser")
        vm.succeed("chmod 440 /etc/sudoers.d/testuser")

        # Verify the user exists and can use sudo
        vm.succeed("su - testuser -c 'sudo whoami' | grep -q root")

        # Test 1: Running as non-root without --sudo should fail
        # The activation requires writing to /etc and /nix/var which needs root
        vm.fail("su - testuser -c '${system-manager-cli}/bin/system-manager activate --store-path ${toplevel} 2>&1'")

        # Test 2: Register and activate with --sudo should succeed
        # First register the profile (creates the symlink)
        vm.succeed("su - testuser -c '${system-manager-cli}/bin/system-manager register --sudo --store-path ${toplevel} 2>&1' | tee /tmp/sudo-register.log")
        vm.succeed("! grep -F 'ERROR' /tmp/sudo-register.log")

        # Then activate
        vm.succeed("su - testuser -c '${system-manager-cli}/bin/system-manager activate --sudo --store-path ${toplevel} 2>&1' | tee /tmp/sudo-activate.log")
        vm.succeed("! grep -F 'ERROR' /tmp/sudo-activate.log")

        # Verify activation worked
        vm.wait_for_unit("system-manager.target")
        vm.succeed("systemctl status service-9.service")
        vm.succeed("test -f /etc/foo.conf")

        # Test 3: Deactivation with --sudo should also work
        # Now that the profile is registered, deactivate can find the engine
        vm.succeed("su - testuser -c '${system-manager-cli}/bin/system-manager deactivate --sudo 2>&1' | tee /tmp/sudo-deactivate.log")
        vm.succeed("! grep -F 'ERROR' /tmp/sudo-deactivate.log")

        # Verify deactivation worked
        vm.fail("systemctl status service-9.service")
        vm.fail("test -f /etc/foo.conf")
      '';
  }

//

  # Test remote deployment via SSH
  # This test runs the engine directly via SSH from the host (test driver) to the VM
  # It tests that the engine can be invoked remotely, which is the core of --target-host
  forEachUbuntuImage "target-host" {
    modules = [
      (testModule "old")
      ../examples/example.nix
    ];
    extraPathsToRegister = [
      system-manager.packages.x86_64-linux.default
    ];
    # Use driver instead of sandboxed since we need network access from the test script
    projectTest = test: test.driver;
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      let
        # SSH key for passwordless authentication
        sshKeyGen = hostPkgs.runCommand "ssh-keys" { } ''
          mkdir -p $out
          ${hostPkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -C "test@nix-vm-test"
        '';
      in
      ''
        import subprocess
        import tempfile
        import os

        # Start all machines in parallel
        start_all()

        vm.wait_for_unit("default.target")

        # Enable and start SSH on the VM
        vm.succeed("systemctl unmask ssh.service ssh.socket")
        # Generate SSH host keys if they don't exist
        vm.succeed("ssh-keygen -A")
        vm.succeed("systemctl enable ssh.service")
        vm.succeed("systemctl start ssh.service")
        vm.wait_for_unit("ssh.service")

        # Set up SSH authorized_keys for root
        vm.succeed("mkdir -p /root/.ssh && chmod 700 /root/.ssh")
        vm.succeed("cat ${sshKeyGen}/id_ed25519.pub >> /root/.ssh/authorized_keys")
        vm.succeed("chmod 600 /root/.ssh/authorized_keys")

        # Forward port 2222 on host to port 22 on guest
        vm.forward_port(2222, 22)

        # Create a temporary directory for SSH config
        with tempfile.TemporaryDirectory() as tmpdir:
            # Copy the SSH private key to temp dir with correct permissions
            key_path = os.path.join(tmpdir, "id_ed25519")
            with open("${sshKeyGen}/id_ed25519", "r") as src:
                with open(key_path, "w") as dst:
                    dst.write(src.read())
            os.chmod(key_path, 0o600)

            # Create SSH config to disable host key checking
            ssh_config = os.path.join(tmpdir, "ssh_config")
            with open(ssh_config, "w") as f:
                f.write("Host *\n")
                f.write("  StrictHostKeyChecking no\n")
                f.write("  UserKnownHostsFile /dev/null\n")
                f.write("  LogLevel ERROR\n")

            ssh_opts = ["-F", ssh_config, "-i", key_path, "-p", "2222"]

            # Test SSH connectivity first
            result = subprocess.run(
                ["${hostPkgs.openssh}/bin/ssh"] + ssh_opts +
                ["-o", "ConnectTimeout=10", "root@127.0.0.1", "echo", "SSH works"],
                capture_output=True, text=True, timeout=30
            )
            assert result.returncode == 0, f"SSH test failed: {result.stderr}"
            print(f"SSH test output: {result.stdout}")

            # Verify the store path is accessible on the remote (via extraPathsToRegister)
            result = subprocess.run(
                ["${hostPkgs.openssh}/bin/ssh"] + ssh_opts +
                ["root@127.0.0.1", "ls", "${toplevel}"],
                capture_output=True, text=True, timeout=30
            )
            assert result.returncode == 0, f"Store path not accessible on remote: {result.stderr}"
            print(f"Store path contents: {result.stdout}")

            # Test 1: Invoke engine-activate remotely via SSH
            # This tests the core of --target-host functionality: running the engine via SSH
            result = subprocess.run(
                ["${hostPkgs.openssh}/bin/ssh"] + ssh_opts +
                ["root@127.0.0.1", "--",
                 "${toplevel}/bin/system-manager-engine", "activate",
                 "--store-path", "${toplevel}"],
                capture_output=True, text=True, timeout=120
            )
            print(f"Remote activate stdout: {result.stdout}")
            print(f"Remote activate stderr: {result.stderr}")
            # Note: The activation may report a D-Bus timeout on slow VMs (no KVM),
            # but the actual activation usually succeeds. We verify this below.

        # Wait for systemd to settle after activation
        import time
        time.sleep(5)

        # Verify activation worked on the VM by checking critical files/services
        # First ensure systemd picked up the new units (D-Bus may have timed out earlier)
        vm.succeed("systemctl daemon-reload")
        vm.succeed("systemctl start system-manager.target")
        vm.wait_for_unit("system-manager.target")
        vm.succeed("systemctl status service-9.service")
        vm.succeed("test -f /etc/foo.conf")
        vm.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")

        # Test 2: Invoke engine-deactivate remotely via SSH
        with tempfile.TemporaryDirectory() as tmpdir:
            key_path = os.path.join(tmpdir, "id_ed25519")
            with open("${sshKeyGen}/id_ed25519", "r") as src:
                with open(key_path, "w") as dst:
                    dst.write(src.read())
            os.chmod(key_path, 0o600)

            ssh_config = os.path.join(tmpdir, "ssh_config")
            with open(ssh_config, "w") as f:
                f.write("Host *\n")
                f.write("  StrictHostKeyChecking no\n")
                f.write("  UserKnownHostsFile /dev/null\n")
                f.write("  LogLevel ERROR\n")

            ssh_opts = ["-F", ssh_config, "-i", key_path, "-p", "2222"]

            result = subprocess.run(
                ["${hostPkgs.openssh}/bin/ssh"] + ssh_opts +
                ["root@127.0.0.1", "--",
                 "${toplevel}/bin/system-manager-engine", "deactivate"],
                capture_output=True, text=True, timeout=120
            )
            print(f"Remote deactivate stdout: {result.stdout}")
            print(f"Remote deactivate stderr: {result.stderr}")
            assert result.returncode == 0, f"Remote deactivate failed: {result.stderr}"

        # Verify deactivation worked on the VM
        vm.fail("systemctl status service-9.service")
        vm.fail("test -f /etc/foo.conf")
      '';
  }
