# Configuration

This guide covers how to organize your System Manager project and recommended workflows for different scenarios.

# Setting up a folder and file structure

Before you begin with System Manager, you'll need to decide on your folder structure.

!!! Note
    If you prefer, you can host all your System Manager configuration files on a remote Git repo (such as GitHub), and then you don't need to worry about where on your computer to store the files. For more info, see [Working with Remote Flakes](../how-to/use-remote-flakes.md).

Technically, you are free to set up your folders and files however you like; System Manager does not enforce any rules, thus allowing you full flexibility. Below are simply some options that we recommend.

!!! Tip
    While you are free to have different System Manager `.nix` files scattered throughout your system, we recommend, if possible, keeping them in a single location simply for organizational purposes. But again, this is just a recommendation and you're not bound by any such rules.

## Deciding on a folder structure

You'll need to choose where your System Manager configuration will live. Here are two main organizational patterns we recommend.

* **Option A**: A single folder for all your configuration

A single folder keeps everything together. This offers a clean long-term solution, along with easy version control. It's also convenient for replicating between machines.

* **Option B**: A separate folder for each use case

While not as common, it's entirely possible to organize your System Manager configuration into multiple independent folders, each focused on a specific use case. In this model, you treat each configuration as its own standalone unit, often stored in its own Git repository.

For example, you might keep:

* a dedicated configuration folder strictly for managing nginx,

* another for custom systemd services,

* another for developer tools,

* and yet another for monitoring and observability packages.

In this manner, you can then build up a system by picking and choosing which services you need for a particular machine, and pull each one down from GitHub.

To make this happen, however, requires careful consideration [as we discuss later](#dealing-with-conflicting-nix-files).

## Choosing a location

### Option 1: Your personal ~/.config folder

If you're managing a system yourself and only you will be using it, one possibility is to put the files in `~/.config/system-manager`.

This approach keeps everything scoped to you and avoids having to place files under `/etc` and, perhaps most importantly, avoids having to use sudo. Here's an example layout:

```
~/.config/system-manager/
  flake.nix
  modules/
    default.nix
```

!!! Tip
    Avoid this location if multiple people use the machine or if this configuration is meant to be shared with a team. Home-directory paths are user-specific and may not make sense across machines.

### Option 2: A shared `/etc/system-manager` folder (Recommended for multi-user or organizational setups)

If you are:

* managing multiple machines,

* part of a team,

* deploying configurations in a corporate or server environment,

* or simply want a clear system-level location,

then `/etc/system-manager` is a great choice. Among the advantages are consistency across all machines; standard within an organization; and treating system manager as a system-level tool rather than a personal configuration. Here's an example layout:

```
/etc/system-manager/
  flake.nix
  modules/
    default.nix
    services.nix
```

## Choosing a file structure

After choosing where your configuration lives, you must decide how to structure the files inside it. And note that while System Manager does not enforce any rules, we do recommend you maintain consistency, especially if you have multiple locations on your computer where you store System Manager `.nix` files.

Essentially, you have two options:

* A single `flake.nix` file

* A reusable `flake.nix` file with one or more separate configuration files that describe what the system will look like.

Within Option B, you can also use our open-source Blueprint product to help you manage your files, which we'll cover shortly.

### Option A: Single `flake.nix` file

This configuration is ideal for:

* Small, simple setups

* Demos and experiments

* One-off configurations

Drawback: This approach doesn't scale well once you need multiple services, multiple hosts, or reusable modules.

### Option B: Flake file with one or more configuration files

This is the structure used by most production setups and by NixOS itself. Your arrangement might look like:

```
system-manager/
  flake.nix
  modules/
    default.nix
    services.nix
    users.nix
```

Or, perhaps you might have separate services, one per file:

```
system-manager/
  flake.nix
  modules/
    default.nix
    service-1.nix
    service-2.nix
    users.nix
```

This also lends itself well to having multiple "recipes". For example, you might want to add nginx and postgres to your system. You might have them preconfigured somewhere, and simply "drop" them in like so:

```
system-manager/
  flake.nix
  modules/
    default.nix
    service-1.nix
    service-2.nix
    users.nix
    nginx.nix
    postgres.nix
```

!!! Tip
    This is the approach we use in our examples in this document. That way each isolated "recipe" is repeatable and can be re-used across multiple systems.


### Dealing with conflicting `.nix` files

If you have multiple flakes throughout your computer, you can run into a situation where one might install some software, and the other might install a different software -- but uninstall what was in the other configuration.

For example; suppose you have one configuration file that includes this list of apps:

```nix
    environment = {
      systemPackages = [
        pkgs.bat
        pkgs.nginx
        pkgs.mysql84
      ];
```

And you run System Manager, which installs the three apps.

Then you separately in another folder have another flake with a different configuration and set of apps:

```nix
    environment = {
      systemPackages = [
        pkgs.hello
        pkgs.postgresql_18
        pkgs.vscode
      ];
```

Then in this folder your run System Manager.

System Manager does not track files, and see this as a changed configuration:

* The configuration **no longer has** `bat`, `nginx`, and `mysql84`.
* The configuration does have `hello`, `postgresql_18`, and `vscode`.

The end result is that System Manager will **remove** `bat`, `nginx`, and `mysql84`, and install `hello`, `postgresql_18`, and `vscode`.

The fix to this problem is to instead have a single main `flake.nix` file, which loads all of the different `.nix` files, allowing you to run System Manager from a single location.

This is because Nix has the ability to merge together objects in separate files into a single object; the above would then merge into:

```nix
      systemPackages = [
        pkgs.bat
        pkgs.nginx
        pkgs.mysql84
        pkgs.hello
        pkgs.postgresql_18
        pkgs.vscode
      ];
```

We describe this technique in the [Multi-file Configuration](../tutorials/multi-file-config.md) tutorial.

# Letting System Manager manage `/etc/nix/nix.conf`

System Manager can optionally manage your `/etc/nix/nix.conf` file for you.

If you have an existing `/etc/nix/nix.conf` file, you'll need to delete it if you want System Manager to manage the file; then run System Manager again. From that point on System Manager will manage the file for you, and you should not make changes to it.

Instead, you'll put the changes in one of your `.nix` files you'll be building to configure System Manager.

# Recommended Workflow for Starting Out

As described previously, System Manager wants to manage your `/etc/nix/nix.conf` file for you, after which you can instead place your configurations directly in the `flake.nix` file, including specifying experimental features.

To do so requires a careful set of steps. Follow these steps precisely when starting out with a fresh system.

!!! Note
    We will first run System Manager to create an initial `flake.nix` and `system.nix` file; we will then delete the `/etc/nix/nix.conf` file, and instead add the flags to the `flake.nix` file. Then we will run System Manager again to start managing your system, including the `/etc/nix/nix.conf` file.

1. Temporarily run System Manager with `init` with experimental features enabled by including the following line in `/etc/nix/nix.conf`; this way `init` will generate a `flake.nix` file:

```ini
experimental-features = nix-command flakes
```

And then running System Manager with the init subcommand:

```sh
nix run 'github:numtide/system-manager' -- init
```

(For this step, do not simply add the flag for experimental features; otherwise `init` won't create the `flake.nix` file.)

2. Under `~/.config/system-manager`, edit the `flake.nix` file, replacing this line:

```nix
  modules = [ ./system.nix ];
```

with this:

```nix
modules = [
    {
        nix.settings.experimental-features = "nix-command flakes";
    }
    ./system.nix
];
```

3. Delete the `/etc/nix/nix.conf` file, optionally backing it up first:

```sh
sudo cp /etc/nix/nix.conf /etc/nix/nix_old # optional
sudo rm /etc/nix/nix.conf
```

4. Run System Manager to initialize your system, with the experimental flags set this one time in the command-line:

```sh
cd ~/.config/system-manager
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake . --sudo
```

System Manager is now managing your system for you, including the `/etc/nix/nix.conf` file. And experimental features are required and turned on through the `flake.nix` file, meaning you do not need to include the `--extra-experimental-features` option when you run System Manager:

```
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

Next, if you want to make sure experimental features are always on, you can add it to your flake.

# Using System Manager in a non-Interactive Setting

If you're running System Manager in a non-interative script, you might run into a problem with the four questions presented when you first run it:

* Do you want to allow configuration setting 'extra-substituters' to be set to 'https://cache.numtide.com' (y/N)?

* Do you want to permanently mark this value as trusted (y/N)?

* Do you want to allow configuration setting 'extra-trusted-public-keys' to be set to 'niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=' (y/N)?

* Do you want to permanently mark this value as trusted (y/N)?

The reason for these questions is Numtide has made pre-built binary versions of System Manager available from our cache, which speeds up performance since your system doesn't have to build System Manager from source. However, this triggers Nix to ask these four questions. You'll most likely want to answer "y" to all four.

But doing so can cause problems with a non-interactive script. To run System Manager in a script, you can simply add the `--accept-flake-config` option like so:

```sh
nix run 'github:numtide/system-manager' --accept-flake-config --extra-experimental-features 'nix-command flakes' -- switch --flake . --sudo
```

If you like, you can add these settings into your flake file, such as in the following:

```nix
{
  description = "Standalone System Manager configuration";

  inputs = {
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
        modules = [
            {
                nix.settings.experimental-features = "nix-command flakes";
                nix.settings.extra-substituters = https://cache.numtide.com;
                nix.settings.extra-trusted-public-keys = niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=;
            }
            ./glow.nix
        ];
      };
    };
}
```

Remember, however, the flake shows what the system looks like *after* System Manager runs. That means these changes won't affect the first run of System Manager, which in this case is likely through a script. As such, the first time you run System Manager, you'll still need the `--accept-flake-config` option. Then on subsequent runs you don't need the `--accept-flake-config` option.

# Recommended Workflow if You Already Have Your Nix Files

If you already have your `.nix` files, you don't need to run the `init` subcommand. Instead, we recommend the following if you're starting out on a clean system:

1. Remove the `/etc/nix/nix.conf` file. Then, when you run System Manager the first time, System Manager will take control managing this file for you. You can then place any configuration you previously had in the `/etc/nix/nix.conf` file in your `.nix` files.

2. Run System Manager the first time, and you'll be ready to go.

As an example, here's a starting `flake.nix` file:

**flake.nix**
```nix
{
  description = "Standalone System Manager configuration";

  inputs = {
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
        modules = [
            {
                nix.settings.experimental-features = "nix-command flakes";
            }
            ./glow.nix
        ];
      };
    };
}
```

Notice that we've included in the modules list an object that sets experimental features, turning on flakes.

Now here's the `glow.nix` file referenced above; it simply installs the `glow` command, which is for displaying markdown files in a shell:

**glow.nix**

```nix
{ pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment.systemPackages = with pkgs; [
        glow
    ];
  };
}
```

Go ahead and delete `/etc/nix/nix.conf`:

```sh
sudo rm /etc/nix/nix.conf
```

And now run System Manager. Because you removed `nix.conf`, you'll need to turn on experimental features as a command-line option.

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake . --sudo
```

After System Manager runs, you'll have the changes in place (in this case the `glow` command added), and you'll be able to manage features, including experimental features, through your flake. And because you turned on the flakes experimental features, future runs of System Manager no longer need the flags. You can simply run:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```
