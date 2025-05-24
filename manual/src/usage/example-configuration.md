# Example Configuration

This is a minimal example which adds the `system-manager` package to the environment, a simple
`say-hello` systemd service, and some arbitrary text file to `/etc`. Other examples used in testing
can be found in the repository's [examples](https://github.com/numtide/system-manager/tree/main/examples) directory.

First, we create a `system.nix` module, which will be the base configuration for our system.

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "aarch64-linux";

    environment = {
      # Packages that should be installed
      # on a system
      systemPackages = with pkgs; [
        git
        nil
        helix
      ];

      # Add directories and files to `/etc`
      # and set their permissions
      etc = {
        my_text_file = {
          text = ''
            Arbitrary text file content.
          '';
        };
      };
    };

    # Create systemd services
    systemd.services.say-hello = {
      description = "say-hello";
      enable = true;
      wantedBy = [ "system-manager.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      script = ''
        ${lib.getBin pkgs.hello}/bin/hello
      '';
    };

    # Configure systemd services
    services.say-hello.enable = true;
  };
}
```

Then we can reference the file path in our flake. For simplicity's sake, we include another
attribute set in `modules` which captures the `system-manager` package as part of the system's
`environment.systemPackages`.

> Because nix is a functional configuration language, this is cumulative
> and the resulting system will include both the `systemPackages` from our `system.nix` as well as
> any other modules we include which refer to `environment.systemPackages`.

```nix
{
  description = "Example System Manager configuration.";

  inputs = {
    # Specify the source of System Manager and Nixpkgs.
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      system-manager,
      ...
    }:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        # Specify your system configuration modules here, for example,
        # the path to your `system.nix`.
        modules = [
          ./system.nix
          ({
            config.environment.systemPackages = [
              system-manager.packages.${system}.system-manager
            ];
          })
        ];
      };
    };
}
```

Now that we have a system configuration that `system-manager` can build, we build and switch
to the configuration in one step:

Ensure that `$PATH` state is used when invoking nix via `sudo`, which is required for switching configurations.

```sh
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake '.#ubuntu'
```

Once the command finishes, the following commands should yeild their expected output:

```sh
system-manager --version
system-manager 0.1.0

git --version
git 2.49.0

nil --version
nil 2024-08-06

hx --version
helix 25.01.1 (e7ac2fcd)

cat /etc/my_text_file
Arbitrary text file content.

systemctl status say-hello.service
● say-hello.service - say-hello
     Loaded: loaded (/etc/systemd/system/say-hello.service; enabled; vendor preset: enabled)
     Active: active (exited) since Wed 2025-05-21 09:39:24 PDT; 11min ago
   Main PID: 41644 (code=exited, status=0/SUCCESS)
        CPU: 3ms

May 21 09:39:24 ubuntu systemd[1]: Starting say-hello...
May 21 09:39:24 ubuntu say-hello-start[41646]: Hello, world!
May 21 09:39:24 ubuntu systemd[1]: Finished say-hello.
```
