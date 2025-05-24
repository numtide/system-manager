# Environment

## Adding Packages

Adding packages with `system-manager` works in the same way that it does on `nixos`, the only difference is that the `system-manager` module expects a `config`.
The list of available config options can be found [here](https://github.com/numtide/system-manager/blob/main/nix/modules/default.nix).

Below is a minimal example `system.nix` which can be imported in via `modules` with `makeSystemConfig`.

```nix
# system.nix
{ pkgs, ... }:
{
  config = {
    environment.systemPackages = with pkgs; [
      cowsay
    ];
  };
}
```

## Managing Files

Managing files in `/etc` with `system-manager` works in the same way that it does on `nixos`, the only difference is that the `system-manager` module expects a `config`.
The list of available config options can be found [here](https://github.com/numtide/system-manager/blob/main/nix/modules/default.nix).

Below is a minimal example `system.nix` which can be imported in via `modules` with `makeSystemConfig`.

```nix
# system.nix
{ ... }:
{
  config = {
    environment.etc = {
      with_ownership = {
        text = ''
          ...
        '';
        mode = "0755";
        uid = 5;
        gid = 6;
      };
    };
  };
}
```
