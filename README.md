<div align="center">

# system-manager

<img src="system-manager.svg" height="150"/>

**Manage system config using nix on any distro**

*A <a href="https://numtide.com/">numtide</a> project.*

<p>
<a href="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml"><img src="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml/badge.svg"/></a>
<a href="https://app.element.io/#/room/#home:numtide.com"><img src="https://img.shields.io/badge/Support-%23numtide-blue"/></a>
</p>

</div>

System Manager brings the power of NixOS-style declarative configuration to other Linux distributions. Describe what your system should look like, by specifying packages, services, and settings all in Nix, then apply it safely and atomically with a single command. Each change is reproducible and soon will be rollback-ready, just like NixOS generations.

If you're familiar with Home Manager, think of it as being similar, but for your entire machine. Whereas Home Manager manages user environments, System Manager manages the entire system, starting at root-level configurations, packages, and services, using the same reliable, Nix-based model.

System Manager builds on the many modules that already exist in [NixOS].

## Quick Example to Get Started

We will assume you're using a non-NixOS distrubution (such as Ubuntu) and you have Nix already installed.

In a folder create a file called `flake.nix` with the following:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        ./modules
      ];
    };
  };
}
```

In that folder create a subfolder called `modules`. In `modules` create a file called
`default.nix` with the following system configuration:

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";
  
  environment.systemPackages = with pkgs; [
    btop          # Beautiful system monitor
    bat           # Modern 'cat' with syntax highlighting
  ];
}

```

This specifies a configuration that includes `btop` and `bat`, to be installed on the system. To do so, execute system manager with `sudo` using the nix command (assuming you have experimental features nix-command and flakes turned on):

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

Notice we're passing the current PATH environment into sudo so that the elevated shell can locate the nix command.

Also, note that you might need to enable nix-commands and flakes:

```
sudo env PATH="$PATH" nix --extra-experimental-features 'nix-command flakes' run 'github:numtide/system-manager' -- switch --flake .
```

> [!Note]
> The first time you run system manager, it will update your path by adding an entry inthe /etc/profile.d folder. For such change to take effect, you need to log out and log back in. However, if you don't want to log out, you can simply source the file:
> `source /etc/profile.d/system-manager-path.sh`

Want to remove a package? Simply remove it or comment it out in the `default.nix` file, and run it again. For example, if you want to remove `bat`, simply update the `default.nix` to the following:

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";
  
  environment.systemPackages = with pkgs; [
    btop          # Beautiful system monitor
    # bat         # Comment out or remove
  ];
}
```

# Full Installation and setup

## Install Nix

System manager itself does not need to be installed; but, you do need to install Nix. (However, you can optionally install system-manager locally if you prefer.)

To install Nix, you can either use your distro's package manager, or use one of the following available options to install Nix.

- [Official Nix Installer][official-installer] - The canonical source for installing nix.
- [Determinate Nix Installer][detsys-installer] - A wrapper around the official installer that has
  SELinux support, and enables flake features by default.

> [!Tip]
> Some Nix users have had difficulty installing Nix on Windows Subsystem for Linux (WSL) using the official installer. If you're using WSL, we recommend using Determinate Nix installer.

## Example: Configuring System Services

The following example demonstrates how to specify a system service and activate it.

As before, create a folder, and place the following `flake.nix` in it:

```
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        ./modules
      ];
    };
  };
}
```

Then create a modules folder under this folder, and place the following inside your default.nix:

```nix
{ lib, pkgs, ... }:
{
  config.systemd.services.say-hello = {
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

Activate it using the same nix command as earlier:

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

This will create a system service called "say-hello" (which comes from the line `config.systemd.services.say-hello`) in a unit file at `/etc/systemd/system/say-hello.service` with the following inside it:

```
[Unit]
Description=say-hello

[Service]
Environment="PATH=/nix/store/xs8scz9w9jp4hpqycx3n3bah5y07ymgj-coreutils-9.8/bin:/nix/store/qqvfnxa9jg71wp4hfg1l63r4m78iwvl9-findutils-4.10.0/bin:/nix/store/22r4s6lqhl43jkazn51f3c18qwk894g4-gnugrep-3.12/bin:
/nix/store/zppkx0lkizglyqa9h26wf495qkllrjgy-gnused-4.9/bin:/nix/store/g48529av5z0vcsyl4d2wbh9kl58c7p73-systemd-minimal-258/bin:/nix/store/xs8scz9w9jp4hpqycx3n3bah5y07ymgj-coreutils-9.8/sbin:/nix/store/qqvfn
xa9jg71wp4hfg1l63r4m78iwvl9-findutils-4.10.0/sbin:/nix/store/22r4s6lqhl43jkazn51f3c18qwk894g4-gnugrep-3.12/sbin:/nix/store/zppkx0lkizglyqa9h26wf495qkllrjgy-gnused-4.9/sbin:/nix/store/g48529av5z0vcsyl4d2wbh9
kl58c7p73-systemd-minimal-258/sbin"
ExecStart=/nix/store/d8rjglbhinylg8v6s780byaa60k6jpz1-unit-script-say-hello-start/bin/say-hello-start 
RemainAfterExit=true
Type=oneshot

[Install]
WantedBy=system-manager.target
```

> [!Tip]
> Compare the lines in the say-hello.service file with the default.nix file to see where each comes from.

## Example: Creating files in the /etc folder

Oftentimes when you're creating a system service, you need to create a configuration file in the /etc file that accompanies the service. System manager allows you to do that as well.

As before, start with the following `flake.nix`:

```
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        ./modules
      ];
    };
  };
}
```

Then, in a subfolder called `modules` place the following in `default.nix`:

```
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
  };
  config.environment.etc = {
    sample_configuration = {
      text = ''
        This is some sample configuration text
      '';
    };
  };
}
```

Run it as usual, and you should see the file now exists:

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
ls /etc -ltr
```

which displays the following:

```
lrwxrwxrwx  1 root  root  45 Nov 13 15:19 sample_configuration -> ./.system-manager-static/sample_configuration
```

And you can view the file:

```
cat /etc/sample_configuration
```

which prints out:

```
This is some sample configuration text
```

### Supported Systems

System Manager is currently only supported on NixOS and Ubuntu. However, it can be used on other
distributions by enabling the following:

```nix
{
  config = {
    system-manager.allowAnyDistro = true;
  }
}
```

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to
work with Open Source projects: <https://numtide.com/contact>

[detsys-installer]: https://github.com/DeterminateSystems/nix-installer
[nixos]: https://nixos.org
[official-installer]: https://nixos.org/download.html
