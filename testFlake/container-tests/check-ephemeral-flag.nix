{
  forEachDistro,
  system,
  system-manager-v1-1-0,
  ...
}:

forEachDistro "check-ephemeral-flag" {
  modules = [
    {
      environment.etc = {
        "a/bar" = {
          text = "bar";
          mode = "0700";
          user = "root";
          group = "root";
        };
      };
    }
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      # Start the container
      start_all()

      # Wait for systemd to be ready
      machine.wait_for_unit("multi-user.target")

      def check_file(path, content):
          file = machine.file(path)
          assert file.exists, f"{path} should exist"
          assert file.is_file, f"{path} should be a file"
          assert file.contains(content), f"{path} should contain {content}"

      # Let's activate the profile with a v0 state file (using an old system-manager checkout)
      activation_logs = machine.succeed("${toplevel}/bin/activate --ephemeral")
      for line in activation_logs.split("\n"):
            assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      with subtest("Verify correct files are created"):
          check_file("/run/etc/a/bar", "bar")
    '';
}
