# Home Manager

This example demonstrates how to manage per-user dotfiles and packages with
[Home Manager](https://github.com/nix-community/home-manager) alongside a
System Manager configuration.
Declare both in the same flake and a single `system-manager switch` will
activate the system-level configuration and run Home Manager for each
configured user.
See the [Home Manager manual](https://nix-community.github.io/home-manager/)
for the full catalogue of options and programs Home Manager can manage.

## Configuration

### flake.nix

```nix
{
  description = "System Manager with Home Manager";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      system-manager,
      home-manager,
      ...
    }:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [
          home-manager.nixosModules.home-manager
          (
            { pkgs, ... }:
            {
              nixpkgs.hostPlatform = "x86_64-linux";

              # Userborn creates the user on activation.
              services.userborn.enable = true;

              # Required so home-manager can invoke nix-store/nix-build
              # during activation.
              nix.enable = true;

              users.groups.alice.gid = 5000;
              users.users.alice = {
                isNormalUser = true;
                uid = 5000;
                group = "alice";
                home = "/home/alice";
                createHome = true;
              };

              home-manager = {
                # Share the system's pkgs (and nixpkgs config) with Home Manager.
                useGlobalPkgs = true;
                # Install per-user packages into /etc/profiles/per-user/<name>.
                useUserPackages = true;
                # Back up any pre-existing dotfiles home-manager would overwrite.
                backupFileExtension = "bak";

                users.alice =
                  { pkgs, ... }:
                  {
                    home.stateVersion = "24.05";

                    home.packages = [ pkgs.ripgrep ];

                    home.file.".config/example/hello.txt".text = ''
                      Hello from home-manager!
                    '';

                    programs.git = {
                      enable = true;
                      userName = "Alice";
                      userEmail = "alice@example.com";
                    };
                  };
              };
            }
          )
        ];
      };
    };
}
```

## Usage

```bash
# Activate system-manager; home-manager runs per user during activation.
nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example --sudo
```

The per-user systemd unit `home-manager-<username>.service` runs once per
activation.
Check its status with:

```bash
systemctl status home-manager-alice.service
```

The activated dotfiles and generation symlinks live under
`/home/alice/.config/` and `/home/alice/.local/state/nix/profiles/`.

## Notes

- Users referenced in `home-manager.users.<name>` must also exist in
  `users.users.<name>`; Home Manager reads `home` and the resolved username
  from the system's user database.
- `useGlobalPkgs = true` disables Home Manager's own `nixpkgs.*` options and
  reuses the instance from System Manager.
- `useUserPackages = true` makes `home.packages` available on the user's
  login shell via `/etc/profiles/per-user/<username>/bin`.
- Home Manager activation requires a working Nix setup on the host
  (`nix.enable = true` or an equivalent multi-user Nix install).
