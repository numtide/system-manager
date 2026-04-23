{ forEachDistro, ... }:

forEachDistro "user-packages" {
  modules = [
    (
      { pkgs, ... }:
      {
        users.users.alice = {
          isNormalUser = true;
          uid = 3001;
          packages = [ pkgs.hello ];
        };
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

      with subtest("per-user profile is installed"):
          machine.succeed("test -d /etc/profiles/per-user/alice")

      with subtest("user packages are available in the profile"):
          resolved = machine.succeed("readlink -f /etc/profiles/per-user/alice/bin/hello").strip()
          assert resolved == "${hostPkgs.hello}/bin/hello", (
              f"Expected hello from ${hostPkgs.hello}, got: {resolved}"
          )

      with subtest("hello is on alice's login shell PATH"):
          which_hello = machine.succeed(
              "su --login alice -c 'command -v hello'"
          ).strip()
          assert which_hello == "/etc/profiles/per-user/alice/bin/hello", (
              f"Expected hello resolved from per-user profile, got: {which_hello}"
          )
          greeting = machine.succeed("su --login alice -c 'hello'").strip()
          assert "Hello, world!" in greeting, f"Expected hello greeting, got: {greeting}"
    '';
}
