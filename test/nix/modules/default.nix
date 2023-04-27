{ lib
, system-manager
, system
}:

lib.flip lib.mapAttrs' system-manager.lib.images.ubuntu.${system} (imgName: image:
lib.nameValuePair "example-${imgName}"
  (system-manager.lib.make-vm-test {
    inherit system;
    modules = [
      ({ config, ... }:
        let
          inherit (config) hostPkgs;
        in
        {
          nodes = {
            node1 = { config, ... }: {
              modules = [
                ../../../examples/example.nix
              ];

              virtualisation.rootImage = system-manager.lib.prepareUbuntuImage {
                inherit hostPkgs image;
                nodeConfig = config;
              };
            };
          };

          testScript = ''
            # Start all machines in parallel
            start_all()

            node1.wait_for_unit("default.target")

            node1.execute("/system-manager-profile/bin/activate")
            node1.wait_for_unit("system-manager.target")

            node1.wait_for_unit("service-9.service")
            node1.wait_for_file("/etc/baz/bar/foo2")
            node1.wait_for_file("/etc/foo.conf")
            node1.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
            node1.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")
          '';
        })
    ];
  })
)
