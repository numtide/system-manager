{ forEachDistro, lib, ... }:

forEachDistro "systemd-path" {
  modules = [
    (
      { pkgs, ... }:
      {
        systemd.services = {
          default-path.serviceConfig.ExecStart = "/bin/true";
          explicit-path = {
            path = [ pkgs.hello ];
            serviceConfig.ExecStart = "/bin/true";
          };
          disabled-default-path = {
            enableDefaultPath = false;
            path = [ pkgs.hello ];
            serviceConfig.ExecStart = "/bin/true";
          };
          empty-path = {
            enableDefaultPath = false;
            serviceConfig.ExecStart = "/bin/true";
          };
        };
      }
    )
  ];
  testScriptFunction =
    { hostPkgs, ... }:
    let
      defaultPath = with hostPkgs; [
        coreutils
        findutils
        gnugrep
        gnused
        systemdMinimal
      ];
      expectedPaths = {
        default-path = defaultPath;
        explicit-path = [ hostPkgs.hello ] ++ defaultPath;
        disabled-default-path = [ hostPkgs.hello ];
        empty-path = [ ];
      };
      expectedLines = lib.mapAttrs (
        _: path:
        lib.optional (path != [ ]) (
          "Environment=\"PATH="
          + lib.concatStringsSep ":" (map (p: "${p}/bin") path ++ map (p: "${p}/sbin") path)
          + "\""
        )
      ) expectedPaths;
    in
    ''
      import json

      start_all()
      machine.wait_for_unit("multi-user.target")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      expected_lines = json.loads(r'${builtins.toJSON expectedLines}')
      for name, expected in expected_lines.items():
          with subtest(f"PATH for {name}"):
              unit = machine.succeed(f"cat /etc/systemd/system/{name}.service")
              actual = [line for line in unit.splitlines() if line.startswith('Environment="PATH=')]
              assert actual == expected, f"Expected {expected!r}, got {actual!r}"
    '';
}
