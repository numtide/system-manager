{ forEachDistro, ... }:

# nixpkgs.flake.source is set automatically by makeSystemConfig, so enabling
# nix is enough to get a pinned nixpkgs registry entry and NIX_PATH.
forEachDistro "nixpkgs-flake" {
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

      activation_logs = machine.activate()
      for line in activation_logs.split("\n"):
          assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      registry_file = machine.file("/etc/nix/registry.json")

      with subtest("/etc/nix/registry.json contains a nixpkgs entry"):
          assert registry_file.exists, "/etc/nix/registry.json should exist"
          import json
          data = json.loads(registry_file.content)
          flakes = data["flakes"]
          nixpkgs_entries = [f for f in flakes if f["from"].get("id") == "nixpkgs"]
          assert len(nixpkgs_entries) == 1, f"Expected one nixpkgs entry, got: {nixpkgs_entries}"
          entry = nixpkgs_entries[0]
          assert entry["to"] == {
              "type": "path",
              "path": "${toplevel.config.nixpkgs.flake.source}",
          }, f"Expected the nixpkgs sources used to build the system, got: {entry}"

      with subtest("NIX_PATH contains nixpkgs=flake:nixpkgs"):
          nix_path = machine.succeed("bash --login -c 'echo $NIX_PATH'").strip()
          assert "nixpkgs=flake:nixpkgs" in nix_path, (
              f"Expected 'nixpkgs=flake:nixpkgs' in NIX_PATH, got: {nix_path!r}"
          )
    '';
}
