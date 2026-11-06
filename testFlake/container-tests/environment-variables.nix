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

      with subtest("variables are emitted by the user environment generator"):
          content = machine.succeed(
              "env -i USER=alice PATH=/usr/bin /etc/systemd/user-environment-generators/50-system-manager"
          )
          assert 'FOO="bar"' in content, f"Expected FOO in generator output, got: {content}"
          assert 'PATHLIKE="/a:/b:/c"' in content, f"Expected PATHLIKE in generator output, got: {content}"
          assert "NULLED" not in content, f"Expected NULLED to be absent, got: {content}"
          assert 'PATH="/etc/profiles/per-user/alice/bin:/run/system-manager/sw/bin:/usr/bin"' in content, (
              f"Expected expanded PATH in generator output, got: {content}"
          )
          assert "/run/system-manager/sw" in content, f"Expected system profile in NIX_PROFILES, got: {content}"

      with subtest("systemd user manager picks up the generator"):
          machine.succeed("systemctl start user@0.service")
          env = machine.succeed("XDG_RUNTIME_DIR=/run/user/0 systemctl --user show-environment")
          assert "FOO=bar" in env.split("\n"), f"Expected FOO in user manager environment, got: {env}"
          assert "PATH=/etc/profiles/per-user/root/bin:/run/system-manager/sw/bin:" in env, (
              f"Expected expanded PATH in user manager environment, got: {env}"
          )

      with subtest("pam_env does not see unexpanded variables"):
          value = machine.succeed("su -s /bin/sh nobody -c 'echo \"$PATH\"'").strip()
          assert "''${" not in value, f"Expected no unexpanded variables in PATH, got: {value}"

      with subtest("NIX_PROFILES is exported in login shell"):
          content = machine.succeed("cat /etc/profile.d/system-manager-path.sh")
          assert "NIX_PROFILES=" in content, f"Expected NIX_PROFILES in profile script, got: {content}"
          assert "/run/system-manager/sw" in content, f"Expected system profile in NIX_PROFILES, got: {content}"
          value = machine.succeed("bash --login -c 'echo $NIX_PROFILES'").strip()
          assert "/run/system-manager/sw" in value.split(), (
              f"Expected /run/system-manager/sw in NIX_PROFILES, got: {value!r}"
          )
          assert "/etc/profiles/per-user/" in value, (
              f"Expected per-user profile in NIX_PROFILES, got: {value!r}"
          )

      with subtest("sessionVariables are login shell exports"):
          value = machine.succeed("bash --login -c 'echo $SESSION_VAR'").strip()
          assert value == "from-session", f"Expected 'from-session', got: '{value}'"

      with subtest("extraSetup removes binary from system PATH"):
          machine.fail("test -e /run/system-manager/sw/bin/hello")
    '';
}
