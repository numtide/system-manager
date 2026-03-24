{
  forEachDistro,
  system-manager,
  system,
  ...
}:

let
  failingToplevel = system-manager.lib.makeSystemConfig {
    modules = [
      (
        { pkgs, ... }:
        {
          nixpkgs.hostPlatform = system;
          system.checks = [
            (pkgs.runCommand "failing-check" { } ''
              echo "this check should fail" >&2
              exit 1
            '')
          ];
        }
      )
    ];
  };
in
forEachDistro "system-checks" {
  modules = [
    (
      { pkgs, ... }:
      {
        system.checks = [
          (pkgs.runCommand "passing-check" { } ''
            echo "check passed" > $out
          '')
        ];
      }
    )
  ];
  extraPathsToRegister = [ ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Check outputs exist in toplevel under checks/"):
          machine.succeed("test -d ${toplevel}/checks")
          # Find the passing-check entry (index depends on other modules adding checks)
          machine.succeed("ls ${toplevel}/checks/ | grep -F passing-check")
          content = machine.succeed("cat ${toplevel}/checks/*-passing-check").strip()
          assert content == "check passed", f"Expected 'check passed', got: {content}"

      with subtest("Failing check prevents toplevel from building"):
          machine.fail("nix-store --realise ${builtins.unsafeDiscardOutputDependency failingToplevel.drvPath}")
    '';
}
