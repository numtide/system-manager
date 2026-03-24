{ forEachDistro, ... }:

forEachDistro "nginx-dhparams" {
  modules = [
    (
      { ... }:
      {
        services.nginx = {
          enable = true;
          sslDhparam = true;
          virtualHosts."localhost" = {
            root = "/var/www";
            locations."/".extraConfig = ''
              return 200 "ok";
            '';
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

      with subtest("Verify nginx is running"):
          assert machine.service("nginx").is_running, "nginx should be running"
    '';
}
