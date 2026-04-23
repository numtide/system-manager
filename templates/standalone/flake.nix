{
  description = "Standalone System Manager configuration";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs = {
    system-manager.url = "github:numtide/system-manager";
  };

  outputs =
    { system-manager, ... }:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [ ./system.nix ];
      };
    };
}
