{ forEachDistro, ... }:

forEachDistro "nix-enabled" {
  modules = [
    (
      { ... }:
      {
        nix.enable = true;
      }
    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Pre-existing nix.conf before activation"):
          assert machine.file("/etc/nix/nix.conf").exists, "/etc/nix/nix.conf should exist before activation"
          original_nix_conf = machine.succeed("cat /etc/nix/nix.conf")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("nix.conf is managed after activation"):
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should exist"
          assert nix_conf.contains("experimental-features"), "nix.conf should contain experimental-features"
          assert nix_conf.contains("nix-command"), "nix.conf should contain nix-command"
          assert nix_conf.contains("flakes"), "nix.conf should contain flakes"

      with subtest("Re-activation succeeds"):
          machine.activate()
          machine.wait_for_unit("system-manager.target")
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should still exist after re-activation"
          assert nix_conf.contains("flakes"), "nix.conf should still contain flakes"

      with subtest("Deactivation restores original nix.conf"):
          machine.succeed("${toplevel}/bin/deactivate")
          restored_nix_conf = machine.succeed("cat /etc/nix/nix.conf")
          assert restored_nix_conf == original_nix_conf, f"nix.conf content differs after deactivation:\n  original: {original_nix_conf!r}\n  restored: {restored_nix_conf!r}"
    '';
}
