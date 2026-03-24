{ forEachDistro, ... }:

forEachDistro "extra-init" {
  modules = [
    (
      { ... }:
      {
        environment.extraInit = ''
          export MY_CUSTOM_VAR="hello-from-extraInit"
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

      with subtest("extraInit code is present in profile script"):
          content = machine.succeed("cat /etc/profile.d/system-manager-path.sh")
          assert "MY_CUSTOM_VAR" in content, f"Expected extraInit content in profile script, got: {content}"

      with subtest("extraInit variable is set in login shell"):
          value = machine.succeed("bash --login -c 'echo $MY_CUSTOM_VAR'").strip()
          assert value == "hello-from-extraInit", f"Expected 'hello-from-extraInit', got: '{value}'"
    '';
}
