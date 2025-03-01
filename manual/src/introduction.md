# Introduction

`system-manager` is a command line tool that uses system configurations written in the nix language
to manage installation of services on Linux distributions, such as Ubuntu, in a similar manner to
`nixos-rebuild` on NixOS systems.

## Hello, System Manager

The following example snippet of nix code:

```nix
{
  environment.systemPackages = [
    pkgs.hello
  ];
  systemd.services.say-hello = {
    description = "say-hello";
    enable = true;
    wantedBy = [ "system-manager.target" ];
    script = ''
      ${lib.getBin pkgs.hello}/bin/hello
    '';
  };
}
```

<!-- TODO: Show the actual module file that results from this -->

Would result in `"Hello, World!"` in the journal of the service `say-hello`.

> Context omitted for clarity, see [Example Configuration](./usage/example-configuration.md)
> for a fully functioning use case.
