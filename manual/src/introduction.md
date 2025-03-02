# Introduction

`system-manager` is a command line tool that uses system configurations written in the nix language
to manage installation of system services on Linux distributions, such as Ubuntu, in a similar manner to
`nixos-rebuild` on NixOS systems.

## Hello, System Manager

<!--
  TODO: People on reddit have tried the example in the readme but it doesn't function
  so we should assume that they will also try to use this to get started. This should
  be tested for usability, even if it is only a snippet, it should be a true reflection
  of all other examples found in the repository. Possibly this snippet could then point
  to an actual functioning example for those who want to try it.
-->

The following snippet of nix code...

```nix
{
  environment.systemPackages = [
    pkgs.hello
  ];
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
}
```

produces a systemd service file.

<!-- TODO: Show the actual module file that results from this -->

When the service is enabled it results in `"Hello, World!"` in the journal of the service `say-hello`.

> Context omitted for clarity, see [Example Configuration](./usage/example-configuration.md)
> for a fully functioning use case.
