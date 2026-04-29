{ forEachDistro, nixpkgs, ... }:

forEachDistro "nixpkgs-flake" {
  modules = [
    (
      { ... }:
      {
        nix.enable = true;
        nixpkgs.flake.source = nixpkgs.outPath or (toString nixpkgs);
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
          assert entry["to"]["type"] == "path", f"Expected path to, got: {entry}"
          assert "/nix/store/" in entry["to"]["path"], (
              f"Expected a store path, got: {entry['to']['path']!r}"
          )

      with subtest("NIX_PATH contains nixpkgs=flake:nixpkgs"):
          nix_path = machine.succeed("bash --login -c 'echo $NIX_PATH'").strip()
          assert "nixpkgs=flake:nixpkgs" in nix_path, (
              f"Expected 'nixpkgs=flake:nixpkgs' in NIX_PATH, got: {nix_path!r}"
          )
    '';
}
