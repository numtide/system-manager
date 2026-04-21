{ forEachDistro, ... }:

forEachDistro "environment-variables" {
  modules = [
    (
      { ... }:
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
    '';
}
