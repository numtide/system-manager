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
    {
      toplevel,
      hostPkgs,
      ...
    }:
    ''
      import json

      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Pre-existing nix.conf before activation"):
          assert machine.file("/etc/nix/nix.conf").exists, "/etc/nix/nix.conf should exist before activation"
          original_nix_conf = machine.succeed("cat /etc/nix/nix.conf")

      with subtest("Pre-existing registry before activation"):
          original_registry = '{"flakes":[],"version":2}'
          machine.succeed(f"printf %s '{original_registry}' > /etc/nix/registry.json")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("nix.conf is managed after activation"):
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should exist"
          assert nix_conf.contains("experimental-features"), "nix.conf should contain experimental-features"
          assert nix_conf.contains("nix-command"), "nix.conf should contain nix-command"
          assert nix_conf.contains("flakes"), "nix.conf should contain flakes"

      with subtest("nix-path is exported in login shells"):
          nix_path = machine.succeed("bash --login -c 'printf %s \"$NIX_PATH\"'").strip()
          assert nix_path == "nixpkgs=flake:nixpkgs", (
              f"Expected configured NIX_PATH, got: {nix_path!r}"
          )

      with subtest("nixpkgs is registered system-wide"):
          registry = json.loads(machine.succeed("cat /etc/nix/registry.json"))
          assert registry == {
              "version": 2,
              "flakes": [{
                  "exact": True,
                  "from": {"id": "nixpkgs", "type": "indirect"},
                  "to": {"path": "${toplevel.config.nixpkgs.flake.source}", "type": "path"},
              }],
          }, f"Unexpected flake registry: {registry!r}"

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
          restored_registry = machine.succeed("cat /etc/nix/registry.json")
          assert restored_registry == original_registry, (
              f"registry.json content differs after deactivation:\n"
              f"  original: {original_registry!r}\n  restored: {restored_registry!r}"
          )
    '';
}
