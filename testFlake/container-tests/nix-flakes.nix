{ forEachDistro, ... }:

forEachDistro "nix-flakes" {
  modules = [
    (
      { ... }:
      {
        nix.enable = true;
        nix.registry.example = {
          from = {
            type = "indirect";
            id = "example";
          };
          to = {
            type = "github";
            owner = "foo";
            repo = "bar";
          };
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

      registry_file = machine.file("/etc/nix/registry.json")

      with subtest("/etc/nix/registry.json exists and is valid JSON"):
          assert registry_file.exists, "/etc/nix/registry.json should exist"
          import json
          data = json.loads(registry_file.content)

      with subtest("registry.json contains the example entry"):
          assert data["version"] == 2, f"Expected version 2, got: {data}"
          flakes = data["flakes"]
          example_entries = [f for f in flakes if f["from"].get("id") == "example"]
          assert len(example_entries) == 1, f"Expected one 'example' entry, got: {example_entries}"
          entry = example_entries[0]
          assert entry["to"]["type"] == "github", f"Expected github to, got: {entry}"
          assert entry["to"]["owner"] == "foo", f"Expected owner foo, got: {entry}"
          assert entry["to"]["repo"] == "bar", f"Expected repo bar, got: {entry}"
    '';
}
