{ forEachDistro, ... }:

forEachDistro "ssh" {
  modules = [
    (
      { pkgs, ... }:
      {
        programs.ssh.enable = true;
        services.openssh.enable = true;
        programs.ssh.knownHosts = {
          "github.com" = {
            publicKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";
          };
          "gitlab.com" = {
            extraHostNames = [
              "gitlab.com"
              "10.0.0.1"
            ];
            publicKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAfuCHKVTjquxvt6CM6tdG4SLp1Btn/nOeHHE5UOzRdf";
          };
        };

        # Meh. Not great, but that's the only way I found to share this with the container.
        environment.etc."privatekey" = {
          source = pkgs.writeText "private-key" ''
            -----BEGIN OPENSSH PRIVATE KEY-----
            b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
            QyNTUxOQAAACDTaOxU7gQHrj8hPQks0u4tiVmRhF1oBAl5+2EkQ9fYBAAAAJh3B3i6dwd4
            ugAAAAtzc2gtZWQyNTUxOQAAACDTaOxU7gQHrj8hPQks0u4tiVmRhF1oBAl5+2EkQ9fYBA
            AAAEBc2BENaT8wrgOp3DsEbvS2Lt0NeTrfVztH9NLLPIE1r9No7FTuBAeuPyE9CSzS7i2J
            WZGEXWgECXn7YSRD19gEAAAAD3BpY25vaXJAYW5hcnJlcwECAwQFBg==
            -----END OPENSSH PRIVATE KEY-----
          '';
          mode = "600";
          user = "root";
          group = "root";
        };

        users.users.root.openssh.authorizedKeys.keys = [
          "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINNo7FTuBAeuPyE9CSzS7i2JWZGEXWgECXn7YSRD19gE"
        ];
      }
    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      # For some reason, the ubuntu image is lacking the ssh host key.
      # It's generated as a postinstall hook, so let's run it again.
      machine.succeed("dpkg-reconfigure openssh-server")

      activation_logs = machine.activate()
      for line in activation_logs.split("\n"):
          assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      with subtest("ssh_known_hosts file exists"):
          known_hosts = machine.file("/etc/ssh/ssh_known_hosts")
          assert known_hosts.exists, "/etc/ssh/ssh_known_hosts should exist"

      with subtest("github.com key is present"):
          content = machine.succeed("cat /etc/ssh/ssh_known_hosts")
          assert "github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl" in content, \
              f"Expected github.com key in known_hosts, got: {content}"

      with subtest("gitlab.com key with extra hostnames is present"):
          content = machine.succeed("cat /etc/ssh/ssh_known_hosts")
          assert "gitlab.com,10.0.0.1 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAfuCHKVTjquxvt6CM6tdG4SLp1Btn/nOeHHE5UOzRdf" in content, \
              f"Expected gitlab.com key with extra hostnames in known_hosts, got: {content}"

      with subtest("ssh_config references known hosts file"):
          ssh_config = machine.file("/etc/ssh/ssh_config")
          assert ssh_config.exists, "/etc/ssh/ssh_config should exist"
          config_content = machine.succeed("cat /etc/ssh/ssh_config")
          assert "GlobalKnownHostsFile" in config_content, \
              f"Expected GlobalKnownHostsFile in ssh_config, got: {config_content}"
          assert "/etc/ssh/ssh_known_hosts" in config_content, \
              f"Expected /etc/ssh/ssh_known_hosts path in ssh_config, got: {config_content}"

      with subtest("ssh server test"):
          machine.wait_for_unit("ssh-system-manager.service")
          sshd_config = machine.file("/etc/ssh/sshd_config")
          assert sshd_config.exists, "/etc/ssh/sshd_config should exist"
          sshd_content = machine.succeed("cat /etc/ssh/sshd_config")
          assert "Subsystem sftp /nix/store/" in sshd_content, \
              "/etc/ssh/sshd_config does not appear to be the system-manager provided one."
          machine.succeed('ssh -i /etc/privatekey -o "StrictHostKeyChecking no" root@localhost echo ok')
          machine.succeed('echo "ls /" | sftp -i /etc/privatekey root@localhost')

      with subtest("dpkg update do not remove system-managed owned files"):
          sshd_sum_before_dpkg = sshd_config.sha256sum
          machine.succeed("dpkg-reconfigure openssh-server --frontend=noninteractive")
          sshd_config_new = machine.file("/etc/ssh/sshd_config")
          assert sshd_config_new.exists, "/etc/ssh/sshd_config should exist"
          assert sshd_config_new.sha256sum == sshd_sum_before_dpkg, \
            "it seems like dpkg overwote /etc/ssh/sshd_config"

      with subtest("deactivation removes known hosts file"):
          machine.succeed("${toplevel}/bin/deactivate")
          machine.fail("test -f /etc/ssh/ssh_known_hosts")
    '';
}
