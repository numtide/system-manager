{ forEachDistro, ... }:

forEachDistro "nix-remote-build" {
  modules = [
    (
      { ... }:
      {
        nix.enable = true;
        nix.distributedBuilds = true;
        nix.buildMachines = [
          {
            hostName = "builder.example.org";
            sshUser = "builder";
            sshKey = "/root/.ssh/id_builder";
            systems = [
              "x86_64-linux"
              "aarch64-linux"
            ];
            maxJobs = 4;
            speedFactor = 2;
            supportedFeatures = [
              "kvm"
              "big-parallel"
            ];
          }
        ];
      }
    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      activation_logs = machine.activate()
      for line in activation_logs.split("\n"):
          assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      machines_file = machine.file("/etc/nix/machines")

      with subtest("/etc/nix/machines exists"):
          assert machines_file.exists, "/etc/nix/machines should exist"

      with subtest("/etc/nix/machines contains the builder spec"):
          content = machines_file.content_string
          assert "ssh://builder@builder.example.org" in content, (
              f"Expected ssh://builder@builder.example.org, got: {content!r}"
          )
          assert "x86_64-linux,aarch64-linux" in content, (
              f"Expected comma-joined systems, got: {content!r}"
          )
          assert "/root/.ssh/id_builder" in content, (
              f"Expected ssh key path, got: {content!r}"
          )
          assert "4 2" in content, (
              f"Expected '4 2' for maxJobs and speedFactor, got: {content!r}"
          )
    '';
}
