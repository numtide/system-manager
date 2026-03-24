# <img alt="System Manager" src="https://banner.numtide.com/banner/numtide/system-manager.svg">

<p>
<a href="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml"><img src="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml/badge.svg"/></a>
<a href="https://app.element.io/#/room/#home:numtide.com"><img src="https://img.shields.io/badge/Support-%23numtide-blue"/></a>
</p>

System manager is a tool to configure Linux machines. Unlike Chef, Puppet and Ansible, it only controls a small subset, and most of its changes are done in an immutable layer, thanks to the power of Nix.

Using NixOS-style declarative configurations, you describe what your system should look like, by specifying packages, services, and settings all in Nix, then apply it safely and atomically with a single command. Each change is reproducible, just like NixOS generations.

You don't need to be an expert in Nix to use it, as its syntax is straightforward. But if you're familiar with Nix and Home Manager, think of System Manager as being similar, but for your entire machine. Whereas Home Manager manages user environments, System Manager manages the entire system, starting at root-level configurations, packages, and services, using the same reliable, Nix-based model.

System Manager builds on the many modules that already exist in [NixOS](https://nixos.org/).

You can find the [full documentation here](https://system-manager.net/main/).

## Quick example

We assume you're using a non-NixOS distribution (such as Ubuntu) and you have [Nix installed](https://system-manager.net/main/how-to/install/) with flakes enabled.

Run the init subcommand to generate a starting configuration in `~/.config/system-manager`:

```
nix run 'github:numtide/system-manager' -- init
```

> [!Note]
> You can optionally run the above under `sudo`, which will place the files under `/root/.config/system-manager`. You might need to pass the path, depending on how you installed Nix:
> `sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- init`

> [!Tip]
> If you have problems running this step, check out our full [Getting Started Guide](https://system-manager.net/main/tutorials/getting-started/).

This creates two files: `flake.nix` and `system.nix`. Here is what `flake.nix` looks like:

```nix
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
```

Open `system.nix` and add packages to the `systemPackages` list:

```diff
      systemPackages = with pkgs; [
-       # hello
+       btop
+       bat
      ];
```

Then apply the configuration:

```
cd ~/.config/system-manager
nix run 'github:numtide/system-manager' -- switch --sudo
```

> [!Tip]
> The first time you run this, Nix will ask whether to trust the Numtide cache substituter. Answer yes — it provides pre-built binaries so you don't have to build everything from source.

> [!Note]
> The first time you run System Manager, it adds an entry in `/etc/profile.d/` to update your `$PATH`. Log out and back in for it to take effect, or run:
> `source /etc/profile.d/system-manager-path.sh`

The two commands `btop` and `bat` should now be available on your system.
Want to remove a package? Simply remove it or comment it out in `system.nix` and run the command again.

## Supported Systems

System Manager is currently only supported on NixOS and Ubuntu. However, it can be used on other distributions by enabling the following:

```nix
{
  config = {
    system-manager.allowAnyDistro = true;
  }
}
```

> [!WARNING]
> This is unsupported and untested. Use at your own risk.

## Supported Nix

Nix should be installed with the [nix-installer](https://github.com/NixOS/nix-installer).
System manager is tested against Nix 2.33.0 and above installed via nix-installer.

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to work with Open Source projects: <https://numtide.com/contact>
