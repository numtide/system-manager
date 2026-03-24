{ forEachDistro, ... }:

forEachDistro "etc-files-with-glob" {
  modules = [
    (
      { pkgs, ... }:
      {
        environment.etc = {
          "fail2ban/action.d".source = "${pkgs.fail2ban}/etc/fail2ban/action.d/*.conf";
          "fail2ban/filter.d".source = "${pkgs.fail2ban}/etc/fail2ban/filter.d/*.conf";
        };
      }
    )
  ];

  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("File from glob is not present"):
          machine.fail("test -f /etc/fail2ban/action.d/dummy.conf")

      machine.activate()

      with subtest("File from glob is present"):
          machine.succeed("test -f /etc/fail2ban/action.d/dummy.conf")
    '';
}
