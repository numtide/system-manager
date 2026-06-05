{ forEachDistro, ... }:

forEachDistro "environment-variables" {
  modules = [
    (
      { pkgs, ... }:
      {
        environment.variables = {
          FOO = "bar";
          PATHLIKE = [
            "/a"
            "/b"
            "/c"
          ];
          NULLED = null;
        };

        environment.sessionVariables = {
          SESSION_VAR = "from-session";
          XDG_DATA_DIRS = [
            "/nix/share1"
            "/nix/share2"
          ];
        };

        environment.systemPackages = [ pkgs.hello ];
        environment.extraSetup = ''
          rm -f $out/bin/hello
        '';
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

      with subtest("string variable is exported in login shell"):
          value = machine.succeed("bash --login -c 'echo $FOO'").strip()
          assert value == "bar", f"Expected 'bar', got: '{value}'"

      with subtest("list variable is colon-joined in login shell"):
          value = machine.succeed("bash --login -c 'echo $PATHLIKE'").strip()
          assert value == "/a:/b:/c", f"Expected '/a:/b:/c', got: '{value}'"

      with subtest("null-valued variable is dropped from profile script"):
          content = machine.succeed("cat /etc/profile.d/system-manager-path.sh")
          assert "FOO" in content, f"Expected FOO in profile script, got: {content}"
          assert "NULLED" not in content, f"Expected NULLED to be absent, got: {content}"

      with subtest("sessionVariables are login shell exports"):
          value = machine.succeed("bash --login -c 'echo $SESSION_VAR'").strip()
          assert value == "from-session", f"Expected 'from-session', got: '{value}'"

      with subtest("sessionVariables append existing values"):
          value = machine.succeed("bash --login -c 'SESSION_VAR=existing; export SESSION_VAR; source /etc/profile.d/system-manager-path.sh; echo $SESSION_VAR'").strip()
          assert value == "from-session:existing", f"Expected 'from-session:existing', got: '{value}'"

      with subtest("sessionVariables with list value append existing values"):
          value = machine.succeed("bash --login -c 'XDG_DATA_DIRS=/host/share; export XDG_DATA_DIRS; source /etc/profile.d/system-manager-path.sh; echo $XDG_DATA_DIRS'").strip()
          assert value == "/nix/share1:/nix/share2:/host/share", f"Expected '/nix/share1:/nix/share2:/host/share', got: '{value}'"

      with subtest("sessionVariables with list value works without pre-existing value"):
          value = machine.succeed("bash --login -c 'unset XDG_DATA_DIRS; source /etc/profile.d/system-manager-path.sh; echo $XDG_DATA_DIRS'").strip()
          assert value == "/nix/share1:/nix/share2", f"Expected '/nix/share1:/nix/share2', got: '{value}'"

      with subtest("environment.variables overwrite existing values"):
          value = machine.succeed("bash --login -c 'FOO=existing; export FOO; source /etc/profile.d/system-manager-path.sh; echo $FOO'").strip()
          assert value == "bar", f"Expected 'bar', got: '{value}'"

      with subtest("extraSetup removes binary from system PATH"):
          machine.fail("test -e /run/system-manager/sw/bin/hello")
    '';
}
