{ lib, system-manager, ... }:

let
  inherit (lib) types;

  pkgsType = lib.mkOptionType {
    name = "nixpkgs";
    description = "An evaluation of Nixpkgs; the top level attribute set of packages";
    check = builtins.isAttrs;
  };

  nodeOptions = { config, name, ... }: {
    options = {
      system.name = lib.mkOption {
        type = types.str;
        default = name;
      };

      modules = lib.mkOption {
        # TODO: can we give a better type here?
        # We want a list of system-manager modules
        type = types.listOf types.raw;
      };

      systemConfig = lib.mkOption {
        # TODO figure out correct type
        type = types.raw;
        internal = true;
        readOnly = true;
      };

      virtualisation = {
        rootImage = lib.mkOption {
          # TODO: figure out the correct type.
          type = types.raw;
        };

        memorySize = lib.mkOption {
          type = types.ints.between 256 (1024 * 128);
          default = 1024;
        };

        cpus = lib.mkOption {
          type = types.ints.between 1 1024;
          default = 2;
        };

        vlans = lib.mkOption {
          type = types.ints.between 1 1024;
          default = 1;
        };

        sharedDirectories = lib.mkOption {
          type = types.attrsOf
            (types.submodule {
              options = {
                source = lib.mkOption {
                  type = types.str;
                };
                target = lib.mkOption {
                  type = types.str;
                };
              };
            });
          default = { };
        };
      };
    };

    config = {
      # Include these shared directories by default, they are used by the test driver.
      virtualisation.sharedDirectories = {
        xchg = {
          source = ''"$TMPDIR"/xchg'';
          target = "/tmp/xchg";
        };
        shared = {
          source = ''"''${SHARED_DIR:-$TMPDIR/xchg}"'';
          target = "/tmp/shared";
        };
      };

      systemConfig = system-manager.lib.makeSystemConfig {
        inherit (config) modules;
      };
    };
  };

in

{
  options = {
    # TODO: figure out correct type
    hostPkgs = lib.mkOption {
      type = pkgsType;
    };

    nodes = lib.mkOption {
      type = types.attrsOf (types.submodule nodeOptions);
      default = { };
    };

    testScript = lib.mkOption {
      type = types.str;
    };
  };
}
