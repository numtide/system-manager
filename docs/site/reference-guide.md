# Reference Guide

To get the most out of System Manager, we're offering this guide to help you make the best decisions based on your particular situation.

# Table of Contents

- [System Requirements](#system-requirements)
- [Installation](#installation)
  - [Regarding Experimental Features](#regarding-experimental-features)
  - [Running under sudo](#running-under-sudo)
  - [Command Line Usage](#command-line-usage)
- [Command-line Options](#command-line-options)
  - [init](#init)
  - [switch](#switch)
  - [register](#register)
  - [build](#build)
  - [deactivate](#deactivate)
  - [pre-populate](#pre-populate)
  - [sudo](#sudo)
- [Setting up a folder and file structure](#setting-up-a-folder-and-file-structure)
  - [Deciding on a folder structure](#deciding-on-a-folder-structure)
  - [Choosing a location](#choosing-a-location)
  - [Choosing a file structure](#choosing-a-file-structure)
  - [Dealing with conflicting .nix files](#dealing-with-conflicting-nix-files)
- [Letting System Manager manage `/etc/nix/nix.conf`](#letting-system-manager-manage-etcnixnixconf)
- [Recommended Workflow for Starting Out](#recommended-workflow-for-starting-out)
- [Using System Manager in a non-Interactive Setting](#using-system-manager-in-a-non-interactive-setting)
- [Recommended Workflow if You Already Have Your Nix Files](#recommended-workflow-if-you-already-have-your-nix-files)
- [Building system-manager .nix files](#building-system-manager-nix-files)
  - [The Main flake.nix File](#the-main-flakenix-file)
- [Managing System Services](#managing-system-services)
  - [Specifying the wantedBy Setting](#specifying-the-wantedby-setting)
- [Managing Software Installations](#managing-software-installations)
  - [Example: Installing a couple apps](#example-installing-a-couple-apps)
- [Working With /etc Files Declaratively](#working-with-etc-files-declaratively)
  - [Example: Creating a file in /etc](#example-creating-a-file-in-etc)
  - [Permissions](#permissions)
  - [Users and Groups](#users-and-groups)
- [Supporting System Services with tmp files and folders](#supporting-system-services-with-tmp-files-and-folders)
- [Working with remote flakes](#working-with-remote-flakes)
  - [What's a flake.lock file?](#whats-a-flakelock-file)
  - [Setting up your project for remote hosting](#setting-up-your-project-for-remote-hosting)
  - [When should you update your flake.nix file?](#when-should-you-update-your-flakenix-file)
  - [Can't System Manager build flake.lock for me?](#cant-system-manager-build-flakelock-for-me)
  - [Ensuring success](#ensuring-success)
  - [Running System Manager with a remote flake](#running-system-manager-with-a-remote-flake)
- [Using Blueprint with System Manager](#using-blueprint-with-system-manager)
  - [Using multiple configuration files with Blueprint](#using-multiple-configuration-files-with-blueprint)
- [Full Examples](#full-examples)
  - [Full Example: Installing PostgreSQL](#full-example-installing-postgresql)
  - [Full Example: Installing Nginx](#full-example-installing-nginx)
  - [Full Example: Installing Nginx for HTTPS with a Secure Certificate](#full-example-installing-nginx-for-https-with-a-secure-certificate)
  - [Full Example: Managing a System that runs Custom Software](#full-example-managing-a-system-that-runs-custom-software)
  - [Live example](#live-example)
- [Optional: Installing System Manager Locally](#optional-installing-system-manager-locally)

- FAQ (Maybe put in its own document)

# Command Line Usage

The basic command looks like this:

```nix
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

This is the most common scenario you'll use.

## Command-line Options

### init

This subcommand creates two initial files for use with system manager, a fully-functional flake.nix, and a system.nix file that contains skeleton code.

#### Command line options

**path:** The path where to create the files. If the path doesn't exist, it will be created.

#### Example

```sh
nix run 'github:numtide/system-manager' -- init
```

This will create the initial files in `~/.config/system-manager`.

```sh
nix run 'github:numtide/system-manager' -- init --path='/home/ubuntu/system-manager'
```

This will create the initial files in `/home/ubuntu/system-manager`.

!!! Note
    Presently, System Manager requires Flakes to be active. If you choose to not include the experimental features line in /etc/nix/nix.conf (and instead use the experimental features command line option), then init will only create a system.nix file, rather than both a flake.nix file and system.nix file. 

### switch

The `switch` subcommand builds and activates your configuration immediately, making it both the current running configuration and the default for future boots. Use it whenever you want to apply your changes.

**Note: Rollbacks are not yet implemented.**

The following two parameters are currently both required:

**--flake**: Specifies a flake to use for configuration.

**--sudo**: Specifies that System Manager can use sudo.

### register

[I'm basing the following strictly on the comments in main.rs. Let me know if it needs further work, and I'm open to suggestions to how to improve it. --jeffrey]

The `register` subcommand builds and registers a System Manager configuration, but does not activate it. Compare this to `switch`, which does everything register does, but then activates it.

### build

[I'm basing the following strictly on the comments in main.rs. Let me know if it needs further work, and I'm open to suggestions to how to improve it. --jeffrey]

The `build` subcommand builds everything needed for a switch, but does not register it.

### deactivate

[I'm basing the following strictly on the comments in main.rs. Let me know if it needs further work, and I'm open to suggestions to how to improve it. --jeffrey]

The `deactivate` deactivates System Manager.

### pre-populate

[I'm basing the following strictly on the comments in main.rs. Let me know if it needs further work, and I'm open to suggestions to how to improve it. --jeffrey]

The `prepopulate` subcommand puts all files defined by the given generation in place, but does not start the services. This is useful in scripts.

### sudo

The sudo subcommand grants sudo access to System Manager, while running under the current user. All created files with be owned by the current user.

# Setting up a folder and file structure

Before you begin with System Manager, you'll need to decide on your folder structure.

!!! Note
    If you prefer, you can host all your System Manager configuration files on a remote Git repo (such as GitHub), and then you don't need to worry about where on your computer to store the files. For more info, see [Working with Remote Flakes](#working-with-remote-flakes).

Technically, you are free to set up your folders and files however you like; System Manager does not enforce any rules, thus allowing you full flexibility. Below are simply some options that we recommend.

!!! Tip
    While you are free to have different system manager .nix files scattered throughout your system, we recommend, if possible, keeping them in a single location simply for organizational purposes. But again, this is just a recommendation and you're not bound by any such rules.

## Deciding on a folder structure

You’ll need to choose where your System Manager configuration will live. Here are two main organizational patterns we recommend.

* **Option A**: A single folder for all your configuration

A single folder keeps everything together. This offers a clean long-term solution, along with easy version control. It's also convenient for replicating between machines.

* **Option B**: A separate folder for each use case

While not as common, it’s entirely possible to organize your System Manager configuration into multiple independent folders, each focused on a specific use case. In this model, you treat each configuration as its own standalone unit, often stored in its own Git repository.

For example, you might keep:

* a dedicated configuration folder strictly for managing nginx,

* another for custom systemd services,

* another for developer tools,

* and yet another for monitoring and observability packages.

In this manner, you can then build up a system by picking and choosing which services you need for a particular machine, and pull each one down from GitHub.

To make this happen, however, requires careful consideration [as we discuss later](#dealing-with-conflicting-nix-files). 

## Choosing a location

### Option 1: Your personal ~/.config folder

If you're managing a system yourself and only you will be using it, one possibility is to put the files in ~/.config/system-manager.

This approach keeps everything scoped to you and avoids having to place files under /etc and, perhaps most importantly, avoids have to use sudo. Here's an example layout:

```
~/.config/system-manager/
  flake.nix
  modules/
    default.nix
```

!!! Tip
    Avoid this location if multiple people use the machine or if this configuration is meant to be shared with a team. Home-directory paths are user-specific and may not make sense across machines.

### Option 2: A shared /etc/system-manager folder (Recommended for multi-user or organizational setups)

If you are:

* managing multiple machines,

* part of a team,

* deploying configurations in a corporate or server environment,

* or simply want a clear system-level location,

then /etc/system-manager is a great choice. Among the advantages are consistency across all machines; standard within an organization; and treating system manager as a system-level tool rather than a personal configuration. Here's an example layout:

```
/etc/system-manager/
  flake.nix
  modules/
    default.nix
    services.nix
```

## Choosing a file structure

After choosing where your configuration lives, you must decide how to structure the files inside it. And note that while system-manager does not enforce any rules, we do recommend you maintain consistency, especially if you have multiple locations on your computer where you store system manager .nix files.

Essentially, you have two options:

* A single flake.nix file

* A reusable flake.nix file with one or more separate configuration files that describe what the system will look like.

Within Option B, you can also use our open-source Blueprint product to help you manage your files, which we'll cover shortly.

### Option A: Single flake.nix file

This configuration is ideal for:

* Small, simple setups

* Demos and experiments

* One-off configurations

Drawback: This approach doesn’t scale well once you need multiple services, multiple hosts, or reusable modules.

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


### Dealing with conflicting .nix files

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

* The configuration **no longer has** bat, nginx, and mysql84.
* The configuration does have hello, postgresql_18, and vscode.

The end result is that System Manager will **remove** bat, nginx, and mysql84, and install hello, postgresql_18, and vscode.

The fix to this problem is to instead have a single main flake.nix file, which loads all of the different .nix files, allowing you to run System Manager from a single location.

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

We describe this technique in [Building System Manager .nix Files](#building-system-manager-nix-files).

# Letting System Manager manage `/etc/nix/nix.conf`

System Manager can optionally manage your `/etc/nix/nix.conf` file for you.

If you have an existing `/etc/nix/nix.conf` file, you'll need to delete it if you want System Manager to manage the file; then run System Manager again. From that point on System Manager will manage the file for you, and you should not make changes to it.

Instead, you'll put the changes in one of your .nix files you'll be building to configure System Manager.

# Recommended Workflow for Starting Out

As described previously, System Manager wants to manage your /etc/nix.conf file for you, after which you can instead place your configurations directly in the flake.nix file, including specifying experimental features.

To do so requires a careful set of steps. Follow these steps precisely when starting out with a fresh system.

!!! Note
    We will first run System Manager to create an initial flake.nix and system.nix file; we will then delete the `/etc/nix/nix.conf` file, and instead add the flags to the flake.nix file. Then we will run System Manager again to start managing your system, inluding the `/etc/nix/nix.conf` file.

1. Temporarily run Run system manager init with experimental features enabled by including the following line in /etc/nix/nix.conf; this way `init` will generate a `flake.nix` file:

```ini
experimental-features = nix-command flakes
```

And then running System Manager with the init subcommand:

```sh
nix run 'github:numtide/system-manager' -- init
```

(For this step, do not simply add the flag for experimental features; otherwise init won't create the flake.nix file.)

2. Under ~/.config/system-manager, edit the `flake.nix` file, replacing this line:

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

3. Delete the /etc/nix.conf file, optionally backing it up first:

```sh
sudo cp /etc/nix/nix.conf /etc/nix/nix_old # optional
sudo rm /etc/nix/nix.conf
```

4. Run System Manager to initialize your system, with the experimental flags set this one time in the command-line:

```sh
cd ~/.config/system-manager
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake . --sudo
```

System Manager is now managing your system for you, including the /etc/nix/nix.conf file. And experimental features are required and turned on through the flake.nix file, meaning you do not need to include the --extra-experimental-features flag when you run System Manager:

```
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

Next, if you want to make sure experimental features are always on, you can add it to your flake.

<!-- [TODO: Another example here] -->

# Using System Manager in a non-Interactive Setting

If you're running System Manager in a non-interative script, you might run into a problem with the four questions presented when you first run it:

* Do you want to allow configuration setting 'extra-substituters' to be set to 'https://cache.numtide.com' (y/N)?

* Do you want to permanently mark this value as trusted (y/N)?

* Do you want to allow configuration setting 'extra-trusted-public-keys' to be set to 'niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=' (y/N)?

* Do you want to permanently mark this value as trusted (y/N)?

The reason for these questions is Numtide has made pre-built binary versions of System Manager available from our cache, which speeds up performance since your system doesn't have to build System Manager from source. However, this triggers Nix to ask these four questions. You'll most likely want to answer "y" to all four.

But doing so can cause problems with a non-interactive script. To run System Manager in a script, you can simply add the --accept-flake-config option like so:

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

Remember, however, the flake shows what the system looks like *after* System Manager runs. That means these changes won't affect the first run of System Manager, which in this case is likely through a script. As such, the first time you run System Manager, you'll still need the `--accept-flake-config` flag. Then on subsequent runs you don't need the `--accept-flake-config flag`.

# Recommended Workflow if You Already Have Your Nix Files

If you already have your .nix files, you don't need to run the init subcommand. Instead, we recommend the following if you're starting out on a clean system:

1. Remove the /etc/nix/nix.conf file. Then, when you run your System Manager the first time, System Manager will take control managing this file for you. You can then place any configuration you previously had in the /etc/etc/nix.conf file in your .nix files.

2. Run System Manager the first time, and you'll be ready to go.

As an example, here's a starting nix.flake file:

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

Now here's the glow.nix file referenced above; it simply installs the `glow` command, which is for displaying markdown files in a shell:

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

go ahead and delete /etc/nix/nix.conf:

```sh
sudo rm /etc/nix/nix.conf
```

And now run System Manager. Because you removed nix.conf, you'll need to turn on experimental features as a command-line option.

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake . --sudo
```

After System Manager runs, you'll have the changes in place (in this case the `glow` command added), and you'll be able to manage features, including experimental features, through your flake. And because you turned on the flakes experimental features, future runs of System Manager no longer need the flags. You can sipmly run:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```


# Building system-manager .nix files

Ready for an example! For this example, we're going to use the following:

* Our files will live in `~/.config/system-manager`

* We'll have two files, one `flake.nix`, and `system.nix`

Note that we'll be using the files generated by the System Manager's `init` subcommand. But to show that we're not locked into that format, later we'll demonstrate a single flake.nix file. Then in the sections that follow, we'll demonstrate how you can further split up your files.

We'll demonstrate how to install an app on your machine, then we'll add another app, then we'll uninstall the first app.

We'll also demonstrate how to move items from your `/etc/nix/nix.conf` file into your System Manager configuration file.

## The Main flake.nix File

We recommend you start with a basic flake.nix file similar to this:

```nix
{
  description = "Standalone System Manager configuration";

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
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        # Specify your system configuration modules here, for example,
        # the path to your system.nix.
        modules = [ ./system.nix ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

This is a typical flake with an `inputs` and an `outputs` section. The inputs loads in `nixpkgs` and `system-manager`. The outputs part has one primary job: It calls System Manager's makeSystemConfig function, passing in any number of .nix modules.

Each module, in turn, must specify a config object, containing configuration settings. These can be in separate files, and Nix will merge them into a single config object that gets passed into `makeSystemConfig`.

Your `config` attribute set can have:

* `nixpkgs.hostPlatform`: This specifies the platform such as nixpkgs.hostPlatform = "x86_64-linux";
* `environment`, consisting of
  * systemPackages
  * etc
* `systemd.services`
* `systemd.tmpfiles`

For example, you could then replace the 

```nix
modules = [ ./system.nix ];
```

line with individual .nix files. For example, you might have one file that installs the `bat` command, and another file that installs the `tree` command.

As an example, let's put these two files in a `modules` folder under the folder holding `flake.nix`. Replace the modules line with this:

```nix
modules = [
  {
    nixpkgs.hostPlatform = "x86_64-linux";
  }
  ./modules/tree.nix
  ./modules/bat.nix
];
```

Then here are the individual "recipe" files.

**modules/bat.nix**

```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.bat
      ];
    };
  };
}
```

**modules/tree.nix**

```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.tree
      ];
    };
  };
}
```

Why take this approach? Because you could, for example, have many different recipes stored in a GitHub repo (or anywhere, really), and you could easily drop them into your system, adding a single line in flake.nix for each. Each one would have their own software installations. And this solves the problem described in [Dealing with Conflicting Nix Files](#dealing-with-conflicting-nix-files)

# Managing System Services

System Manager lets you manage systemd services declaratively, using the same module language you used for installing packages or creating files under /etc. Instead of manually placing service files in /etc/systemd/system or enabling them with systemctl, you describe the service in a Nix module—its command, environment, dependencies, restart behavior, and any timers or sockets it needs.

System Manager then generates the correct systemd unit files, installs them into the right directory, and reloads systemd automatically during a switch. This approach gives you repeatability and safety: if you rebuild a machine, the same services come back exactly as before; if a service configuration breaks, you simply roll back to the previous generation. Declarative service management also avoids drift—no accidental edits, no forgotten manual steps, and no inconsistencies between machines or team members.

Using this approach, instead of manually saving a file in `/etc/systemd/system` and then manually starting and stopping the service, you use a `.nix` file to *declaratively* state what you want the service to look like, and that you want it to be active.

Then you can take this same `.nix` file, place it on another system, and run System Manager again, and you'll have the service installed in a way that's identical to the first system.


The following example demonstrates how to specify a system service and activate it.

We're assuming you're using a flake.nix similar to what's found in [The Main flake.nix File](#the-main-flakenix-file).


```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    
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
  };
}
```

Note:

This line is required in the above example:

```nix
wantedBy = [ "system-manager.target" ];
```

(There are other options for wantedBy; we discuss it in full in our Reference Guide under [Specifying WantedBy Setting](./reference-guide.md#specifying-the-wantedby-setting))

Activate it using the same nix command as earlier:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

This will create a system service called `say-hello` (the name comes from the line `config.systemd.services.say-hello`) in a unit file at `/etc/systemd/system/say-hello.service` with the following inside it:

```systemd
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

!!! Tip
    Compare the lines in the `say-hello.service` file with the `say_hello.nix` file to see where each comes from.

You can verify that it ran by running journalctl:

```sh
journalctl -n 20
```

and you can find the following output in it:

```log
Nov 18 12:12:51 my-ubuntu systemd[1]: Starting say-hello.service - say-hello...
Nov 18 12:12:51 my-ubuntu say-hello-start[3488278]: Hello, world!
Nov 18 12:12:51 my-ubuntu systemd[1]: Finished say-hello.service - say-hello.
```

!!! Note
    If you remove the `./apps.nix` line from the `flake.nix`, System Manager will see that the configuration changed and that the apps listed in it are no longer in the configuration. As such, it will uninstall them. This is normal and expected behavior.


## Specifying the wantedBy Setting

The wantedBy attribute tells systemd when to automatically start a service. System Manager includes its own systemd target that you can use in the wantedBy setting to automatically start any services immediately after applying the changes, as well as after reboot. Here's an example wantedBy line in a .nix configuration file:

```nix
wantedBy = [ "system-manager.target" ];
```

(By allowing the service to start after applying changes, you don't need to reboot for the service to start.)

But you're not limited to just this target. For example, if you're creating a system service that runs on a schedule, you might use this:

```nix
wantedBy = [ "timers.target" ]
```

# Managing Software Installations

System Manager allows you to install software in a fully declarative way similar to installing system services. Instead of relying on a traditional package manager and running commands like apt install or dnf install, you list the packages you want in your configuration file. During a switch, System Manager builds a new system profile that includes those packages, activates it, and ensures the software is available on your PATH. This makes installations reproducible and version-controlled. If you reinstall your operating system or set up a new machine, the exact same tools will appear automatically. And because software installation is tied to your configuration (not to manual actions), System Manager prevents drift—no forgotten tools, no mismatched versions across machines, and no surprises when you rollback or update.

!!! Note
    To install software, you add attributes to the `config.environment.systemPackages` attribute set.

## Example: Installing a couple apps

Starting with a flake such as this:

```nix
{
  description = "Standalone System Manager configuration";

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
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        # Specify your system configuration modules here, for example,
        # the path to your system.nix.
        modules = [
          ./apps.nix
        ];
      };
    };
}
```

Notice this flake references a file called apps.nix. In that file we'll add to the systemPackages attribute set. Here's the apps.nix file:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.hello
        pkgs.bat
      ];
    };
  };
}
```

When you run System Manager, you should have the packages called `hello` and `bat` available.

```console
$ which hello
/run/system-manager/sw/bin//hello
$ which bat
/run/system-manager/sw/bin//bat
```

!!! Note
    The first time you install an app through System Manager, System Manager will add a file inside `/etc/profile.d`. This file adds on the `/run/system-manager/sw/bin/` to a user's path when they log in. If this is the first time you've installed an app on this system with System Manager, you'll need to either source that file, or simply log out and log back in.

If you prefer, you can combine the above two .nix files into a single flake:

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
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [
          ({ lib, pkgs, ... }: {
            config = {
              nixpkgs.hostPlatform = "x86_64-linux";
              environment.systemPackages = [
                pkgs.hello
                pkgs.bat
              ];
            };
          })
        ];
      };
    };
}
```

# Working With /etc Files Declaratively

Many applications and services rely on configuration files stored under /etc, and System Manager lets you manage those files declaratively as well. Instead of manually editing files like /etc/some_config, you define them in your Nix configuration and let System Manager write them during a switch. This ensures that your system state is always consistent with your configuration and avoids accidental edits or configuration drift. If you ever rebuild your machine, those files are recreated exactly as before, including permissions, contents, and paths. And because System Manager keeps previous generations, you can safely roll back to earlier versions of /etc files if needed. Declarative /etc management is especially powerful in shared or multi-machine environments, where consistency and repeatability matter most.

Oftentimes, when you're creating a system service, you need to create a configuration file in the `/etc` directory that accompanies the service. System manager allows you to do that as well.

!!! Note
    To install software, you add attributes to the `config.environment.etc` attribute set.

## Example: Creating a file in /etc

Starting with a flake such as this:

```nix
{
  description = "Standalone System Manager configuration";

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
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [
          ./files1.nix
        ];
      };
    };
}
```

Notice this references a file called `files1.nix`. To create files, you add attributes to the config.environment.etc attribute set as follows:

```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      etc = {
        "test/test2/something.txt" = {
          text = ''
            This is just a test!!
          '';
          mode = "0755";
          user = "ubuntu";
          group = "ubuntu";
        };
      };
    };
  };
}
```

This creates a single file inside the folder `/etc/test/test2/` and the file is called `something.txt`.

After running the above with System Manager, you can verify the file exists:

```console
$ cat /etc/test/test2/something.txt
This is just a test!!
```

Note that if you prefer, you can combine the above flake and separate .nix file into a single flake like so:

```nix
{
  description = "Standalone System Manager configuration";
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
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [
          {
            config.nixpkgs.hostPlatform = "x86_64-linux";
            config.environment.etc."test/test2/something.txt" = {
                text = ''
                  This is just a test!!!
                '';
                mode = "0755";
                user = "ubuntu";
                group = "ubuntu";
            };
          }
        ];
      };
    };
}
```

## Permissions

NixOS uses the standard modes of file permissions, consisting of three octal digits; the first represents the user; the second represents the group; the third represents all other users (sometimes called "world" or "others").

Each digit is the sum of the permissions it grants:

* 4 = read (r)
* 2 = write (w)
* 1 = execute (x)

So "0755" means:

* 7 (4+2+1) = owner can read, write, and execute
* 5 (4+1) = group can read and execute
* 5 (4+1) = others can read and execute

Common examples:

**"0644"** = owner can read/write, everyone else can only read

**"0755"** = owner can do everything, everyone else can read and execute

**"0400"** = owner can only read, nobody else can do anything

**"0600"** = owner can read/write, nobody else can touch it

## Users and Groups

To specify a user and group as owners for a file, you can either use the user ID and group ID, or the user name and group name. Here's an example that uses user ID and group ID (notice we set `uid` and `gid`):

```nix
with_ownership = {
  text = ''
    This is just a test!
  '';
  mode = "0755";
  uid = 5;
  gid = 6;
};
```

And here's an example that uses named user and group (notice we set `user` and `group`):

```nix
with_ownership2 = {
  text = ''
    This is just a test!
  '';
  mode = "0755";
  user = "nobody";
  group = "users";
};
```

!!! Tip
    This use of `uid`/`gid` for IDs and `user`/`group` for names aligns with NixOS standards.

# Supporting System Services with tmp files and folders

Some systemd services need runtime directories, temporary files, or specific filesystem structures to exist before they can start. The `systemd.tmpfiles` configuration provides a declarative way to create these files and directories, set their permissions and ownership, and manage cleanup policies. This is particularly useful for volatile directories like those under `/var/run`, `/tmp`, or custom application directories that need to be recreated on each boot with the correct permissions.

For example, if you're running a web application that stores temporary uploads in `/var/app/uploads`, you can use tmpfiles to ensure this directory exists with the correct permissions when the system boots. Without tmpfiles, your service might fail to start because the directory doesn't exist yet, or it might have the wrong ownership and your application can't write to it.

For this we offer two distinct syntaxes you can use, depending on your needs, as shown in the following sample code:

```nix
    # Configure systemd tmpfile settings
    systemd.tmpfiles = {
       rules = [
         "D /var/tmp/system-manager 0755 root root -"
       ];
      
       settings.sample = {
         "/var/tmp/sample".d = {
           mode = "0755";
         };
       };
    };
```

The first example ("rules"), creates a directory called `/var/tmp/system-manager` with mode 0755, owned by user root and group root. (The - means no aged-based cleanup.)

The second example creates the same type of directory at `/var/tmp/sample` with mode 0755, but uses the structured "settings" format. Since user and group aren't specified, they default to root. This Nix-friendly syntax is more readable and easier to maintain than raw tmpfiles.d strings.

# Working with remote flakes

Instead of saving your System Manager configuration files locally, you can optionally keep them in a remote Git repository, such as on GitHub.

!!! Note
    This is a great option if you plan to use the files on multiple machines.

In order to store them on a remote repo, it's imperative that you keep your flake.lock file up to date. 

## What's a flake.lock file?

A flake.lock file is a JSON file that stores the exact versions of all the inputs your flake file depends on, including things like nixpkgs, System Manager itself, and anything else you might import. Instead of pulling the latest version every time you build, the lock file ensures that the same inputs are used consistently across machines and over time. This makes your configuration reproducible, stable, and rollback-friendly. When you do want to update to new versions, you run a command like nix flake update, which refreshes the lock file in a controlled way.

## Setting up your project for remote hosting

As you create your flake.nix and set up any supporting files, you'll want to test it out thoroughly before pushing it up to a remote repo.

For this you have a couple options; one is to test it out on the machine you're currently using. However, we recommend against this, as there might be artifacts on your computer that can interfere with the configuration.

Instead, we recommend starting with a fresh machine. One option is to spin up an EC2 instance on AWS; another is to open up a Virtual Box session on your computer.

!!! Important
    You'll need to ensure you have at least 16GB of disk space on the virtual machine. If you go with 8GB, you're going to run out of space.

After starting with a fresh machine, install Nix, copy over your flake.nix and supporting files, and test it out. Once you're ready, make sure your flake.lock file is up to date. You can create or update the flake.nix file by typing:

```sh
nix flake update
```

And make sure you've pushed it up to the repo. (If you don't do this step, nix will try to build a flake.lock, but will be unable to write it to the same location as the other files, and will error out.)

[todo: Let's create a repo under numtide for this instead of using mine --jeffrey]

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake git+https://github.com/frecklefacelabs/system-manager-test#default --sudo
```

### When should you update your flake.nix file?

Generally, you only need to update your flake.nix file when you want newer versions of your inputs (nixpkgs, etc). Updating isn't necessary for daily use; your configuration will continue to work with the locked versions. But you will want to update your flake.lock file in cases such as:

* You want newer package versions (e.g. newer `btop`, etc.)
* You want security patches 
* You've added new imputs to your flakes (in which case you'll be required to update `flake.lock`)
* You're preparing a fresh install and decide this is a good time to upgrade everything

### Can't System Manager build flake.lock for me?

Yes, but only if the flake.nix file is local to your machine. The problem is System Manager will try to write a flake.lock file in the same location as the flake.nix file, which isn't possible (at this time) with a GitHub repo.



### Ensuring success

In order to ensure System Manager retrieves the correct .nix files from your repo, we recommend including either a branch or a tag along with your repo.



## Running System Manager with a remote flake

!!! Tip
    Before you run this command, we recommend that you nevertheless create a folder to run it from, such as ~/.config/system-manager. 


# Using Blueprint with System Manager

Blueprint is an opinionated library that maps a standard folder structure to flake outputs, allowing you to divide up your flake into individual files across these folders. This allows you to modularize and isolate these files so that they can be maintained individually and even shared across multiple projects.

Blueprint has bulit-in support for System Manager, which means:

* You do not need to call system-manager.lib.makeSystemConfig; Blueprint calls this for you
* You must follow Blueprint's folder structure by placing your files under the hosts folder, and you must name your files `system-configuration.nix`.
* You can have multiple folders under the `hosts` folder (but one level deep), and you can access these using the standard nix specifier, e.g. `.#folder-name.`

In this section we should you how to use Blueprint with System Manager.

Blueprint provides its own initialization that you can start with if you don't already have a flake.nix file using Blueprint. The command to type is:

```sh
nix flake init -t github:numtide/blueprint
```

This results in the following flake:

```nix
{
  description = "Simple flake with a devshell";

  # Add all your dependencies here
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";
    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.follows = "nixpkgs";
  };

  # Load the blueprint
  outputs = inputs: inputs.blueprint { inherit inputs; };
}
```

Now add System Manager to its inputs section:

```nix
    system-manager = {
        url = "github:numtide/system-manager";
        inputs.nixpkgs.follows = "nixpkgs";
    };
```

Next, create a folder called hosts, and under that a folder called default:

```sh
mkdir -p hosts/default
cd hosts/default
```

Inside `default` is where you'll put your configuration file.

**This configuration file must be named `system-configuration.nix**.

For example, here's a configuration file that installs `bat`:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.bat
      ];
    };
  };
}
```

!!! Note
   Notice that we need to include nixpkgs.hostPlatform in this file, as there's no place to include it in the parent `flake.nix` file.

Now return to the folder two levels up (the one containing flake.nix) and you can run System Manager:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

!!! Remember
    As mentioned elsewhere, if this is the first time running System Manager on this computer, you'll need to log out and log back in to pick up the new path.

Then you should find `bat` on your path:

```console
$ which bat
/run/system-manager/sw/bin//bat
```

The default folder is called default; you can also refer to folders by name as mentioned earlier.

If, for example, under the `hosts` folder you have a folder called `tree`, and inside `tree` create a file called `system-configuration.nix` with the following contents:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.tree
      ];
    };
  };
}
```

Then you can choose to install tree by specifying the tree folder like so:

```sh
nix run 'github:numtide/system-manager' -- switch --flake '.#tree' --sudo
```

## Using multiple configuration files with Blueprint

If you want to load multiple configuration files at once, you can create a special system-configuration.nix file that loads multiple files from a `modules` folder (or any name you choose). To accomplish this, create a folder under hosts; for example, you might name it cli-tools. Starting in the folder with flake.nix:

```sh
mkdir -p hosts/cli-tools/modules
```

Then inside the `cli-tools` folder, create a `system-configuration.nix` file with the following:

```nix
{ config, lib, pkgs, ... }:
{
  # Import all your modular configs - they auto-merge! ✨
  imports = [
    ./modules/tldr.nix
    ./modules/cowsay.nix
  ];

  # Base configuration that applies to everything
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
  };
}
```

(Notice this time we can put the nixpkgs.hostPlatform in a single place. As such we won't need it in the configuration files.)

Now move into the `modules` folder:

```sh
cd modules
```

And create two files here:

tldr.nix:

```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.tldr
      ];
    };
  };
}
```

cowsay.nix:
```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.cowsay
      ];
    };
  };
}
```

Now you can return to the top level where you flake.nix file is and run these two configuration files:

```sh
nix run 'github:numtide/system-manager' -- switch --flake '.#cli-tools' --sudo
```

This means if you want to include various recipes, you can easily do so.


# Full Examples

## Full Example: Installing PostgreSQL

Here's a .nix file that installs PostgreSQL.

Note: System Manager is still in its early state, and doesn't yet have user management, which is a planned feature that will be here soon. As such, for now, before you run this, you'll need to manually create the postgres user. Additionally, go ahead and create two directories and grant the postgres user access to them:

```sh
# Create postgres user and group
sudo groupadd -r postgres
sudo useradd -r -g postgres -d /var/lib/postgresql -s /bin/bash postgres

# Create directories with proper permissions
sudo mkdir -p /var/lib/postgresql
sudo chown postgres:postgres /var/lib/postgresql

sudo mkdir -p /run/postgresql
sudo chown postgres:postgres /run/postgresql
```

Here, then, is the .nix file.

```nix
{ config, lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment.systemPackages = with pkgs; [
      postgresql_16
    ];

    # PostgreSQL service
    systemd.services.postgresql = {
      description = "PostgreSQL database server";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      serviceConfig = {
        Type = "notify";
        User = "postgres";
        Group = "postgres";
        ExecStart = "${pkgs.postgresql_16}/bin/postgres -D /var/lib/postgresql/16";
        ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        KillMode = "mixed";
        KillSignal = "SIGINT";
        TimeoutSec = 120;

        # Create directories and initialize database
        ExecStartPre = [
          "${pkgs.coreutils}/bin/mkdir -p /var/lib/postgresql/16"
          "${pkgs.bash}/bin/bash -c 'if [ ! -d /var/lib/postgresql/16/base ]; then ${pkgs.postgresql_16}/bin/initdb -D /var/lib/postgresql/16; fi'"
        ];
      };

      environment = {
        PGDATA = "/var/lib/postgresql/16";
      };
    };

    # Initialize database and user
    systemd.services.postgresql-init = {
      description = "Initialize PostgreSQL database for myapp";
      after = [ "postgresql.service" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        User = "postgres";
      };
      script = ''
        # Wait for PostgreSQL to be ready
        until ${pkgs.postgresql_16}/bin/pg_isready; do
          echo "Waiting for PostgreSQL..."
          sleep 2
        done

        # Optional: Create database if it doesn't exist
        ${pkgs.postgresql_16}/bin/psql -lqt | ${pkgs.coreutils}/bin/cut -d \| -f 1 | ${pkgs.gnugrep}/bin/grep -qw myapp || \
          ${pkgs.postgresql_16}/bin/createdb myapp

        # Optional: Create user if it doesn't exist
        ${pkgs.postgresql_16}/bin/psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='myapp'" | ${pkgs.gnugrep}/bin/grep -q 1 || \
          ${pkgs.postgresql_16}/bin/createuser myapp

        # Grant database privileges
        ${pkgs.postgresql_16}/bin/psql -c "GRANT ALL PRIVILEGES ON DATABASE myapp TO myapp"

        # Grant schema privileges (allows creating tables!)
        ${pkgs.postgresql_16}/bin/psql -d myapp -c "GRANT ALL ON SCHEMA public TO myapp"
        ${pkgs.postgresql_16}/bin/psql -d myapp -c "GRANT ALL ON ALL TABLES IN SCHEMA public TO myapp"
        ${pkgs.postgresql_16}/bin/psql -d myapp -c "GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO myapp"

        echo "PostgreSQL is ready and configured!"
      '';
    };
  };
}
```

## Full Example: Installing Nginx

Here's a .nix file that installs and configures nginx as a system service. Note that this version only supports HTTP and not HTTPS; later we provide an example that includes HTTPS.

!!! Tip
    This is simply an example to help you learn how to use System Manager. The usual way to install nginx under Nix is to use the [nginx package](https://search.nixos.org/packages?channel=25.11&show=nginx&query=nginx).

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Enable and configure services
    services = {
      nginx.enable = true;
    };

    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.hello
        pkgs.mariadb
        pkgs.nginx
      ];

      # Add directories and files to `/etc` and set their permissions
      etc = {
        "nginx/nginx.conf"= {

                user = "root";
                group = "root";
                mode = "0644";

                text = ''
# The user/group is often set to 'nginx' or 'www-data',
# but for a simple root-only demo, we'll keep the default.
# user nginx;
worker_processes auto;

# NGINX looks for modules relative to the install prefix,
# but we explicitly point to the Nix store path to be safe.
error_log /var/log/nginx/error.log;
pid /run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include             ${pkgs.nginx}/conf/mime.types;
    default_type        application/octet-stream;

    sendfile            on;
    keepalive_timeout   65;

    # Basic default server block
    server {
        listen 80;
        server_name localhost;

        # Point the root directory to a standard location or a Nix store path
        root ${pkgs.nginx}/html;

        location / {
            index index.html;
        }

        # Example log files
        access_log /var/log/nginx/access.log;
        error_log /var/log/nginx/error.log;
    }
}
    '';


        };
      };
    };

    # Enable and configure systemd services
    systemd.services = {
        nginx = {
            enable = true;
            description = "A high performance web server and reverse proxy server";
            wantedBy = [ "system-manager.target" ];
            preStart = ''
                mkdir -p /var/log/nginx
                chown -R root:root /var/log/nginx # Ensure permissions are right for root user
            '';
            serviceConfig = {
                Type = "forking";
                PIDFile = "/run/nginx.pid";

                # The main binary execution command, pointing to the Nix store path
                ExecStart = "${pkgs.nginx}/bin/nginx -c /etc/nginx/nginx.conf";

                # The command to stop the service gracefully
                ExecStop = "${pkgs.nginx}/bin/nginx -s stop";

                # NGINX needs to run as root to bind to port 80/443
                User = "root";
                Group = "root";

                # Restart policy for robustness
                Restart = "on-failure";
            };
        };
    };


  };
}

```

## Full Exapmle: Installing Nginx with for HTTPS with a Secure Certificate

Here's an example that installs nginx. This exapmle shows places where you would copy in your own secure certificate information.

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    
    # Enable and configure services
    # Commenting this out -- apparently this loads a bunch of nginx service files we don't need or want
    #services = {
    #  nginx.enable = true;
    #};
    
    environment = {
      systemPackages = [
        pkgs.hello
        pkgs.mariadb
        pkgs.nginx
      ];
      
      # Add SSL certificate files to /etc
      etc = {
        # SSL Certificate
        "ssl/certs/your-domain.crt" = {
          user = "root";
          group = "root";
          mode = "0644";
          # Option 1: Embed the certificate directly
          text = ''
-----BEGIN CERTIFICATE-----
MIIDwzCCAqugAwIBAgIUXbQ2ie2/2pxLH/okEB4KEbVDqjEwDQYJKoZIhvcNAQEL...
-----END CERTIFICATE-----
          '';
          # Option 2: Or reference a file from your repo
          # source = ./certs/your-domain.crt;
        };
        
        # SSL Private Key
        "ssl/private/your-domain.key" = {
          user = "root";
          group = "root";
          mode = "0600";  # Restrict access to private key!
          # Option 1: Embed the key directly
          text = ''
-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC5gQjZxG7rYPub....
-----END PRIVATE KEY-----
          '';
          # Option 2: Or reference a file from your repo
          # source = ./certs/your-domain.key;
        };
        
        # Optional: Certificate chain/intermediate certificates
        # For this demo we're using a self-signed cert; for a real
        # one, uncomment below and add your 
        "ssl/certs/chain.pem" = {
          user = "root";
          group = "root";
          mode = "0644";
          text = ''
            -----BEGIN CERTIFICATE-----
YOUR_CHAIN_CERTIFICATE_HERE...
            -----END CERTIFICATE-----
          '';
        #};
        
        # Nginx configuration with HTTPS
        "nginx/nginx.conf" = {
          user = "root";
          group = "root";
          mode = "0644";
          text = ''
worker_processes auto;

error_log /var/log/nginx/error.log;
pid /run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include             ${pkgs.nginx}/conf/mime.types;
    default_type        application/octet-stream;
    
    sendfile            on;
    keepalive_timeout   65;
    
    # SSL Settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';
    
    # HTTP Server - Redirect to HTTPS
    server {
        listen 80;
        server_name demo.frecklefacelabs.com www.demo.frecklefacelabs.com;
        
        # Redirect all HTTP to HTTPS
        return 301 https://$server_name$request_uri;
    }
    
    # HTTPS Server
    server {
        listen 443 ssl;
        server_name demo.frecklefacelabs.com www.demo.frecklefacelabs.com;
        
        # SSL Certificate files
        ssl_certificate /etc/ssl/certs/your-domain.crt;
        ssl_certificate_key /etc/ssl/private/your-domain.key;
        
        # Optional: Certificate chain
        # ssl_trusted_certificate /etc/ssl/certs/chain.pem;
        
        # Optional: Enable OCSP stapling
        ssl_stapling on;
        ssl_stapling_verify on;
        
        # Optional: Enable HSTS (HTTP Strict Transport Security)
        add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
        
        root ${pkgs.nginx}/html;
        
        location / {
            index index.html;
        }
        
        access_log /var/log/nginx/access.log;
        error_log /var/log/nginx/error.log;
    }
}
          '';
        };
      };
    };
    
    systemd.services = {
      nginx = {
        enable = true;
        #description = "A high performance web server and reverse proxy server";
        wantedBy = [ "system-manager.target" ];
        preStart = ''
          mkdir -p /var/log/nginx
          chown -R root:root /var/log/nginx
          
          # Verify SSL certificate files exist
          if [ ! -f /etc/ssl/certs/your-domain.crt ]; then
            echo "ERROR: SSL certificate not found!"
            exit 1
          fi
          if [ ! -f /etc/ssl/private/your-domain.key ]; then
            echo "ERROR: SSL private key not found!"
            exit 1
          fi
        '';
        serviceConfig = {
          Type = "forking";
          PIDFile = "/run/nginx.pid";
          ExecStart = "${pkgs.nginx}/bin/nginx -c /etc/nginx/nginx.conf";
          ExecStop = "${pkgs.nginx}/bin/nginx -s stop";
          User = "root";
          Group = "root";
          Restart = "on-failure";
        };
      };
    };
  };
}

```

## Full Example: Managing a System that runs Custom Software

Here's an example where you might have custom web software living in a repository and you want to run the software on a system behind nginx.

## Live example

We have a complete example live that you can try out. All you need is a fresh server (such as on Amazon EC2) with at least 16GB memory. (We recommend the latest Ubuntu, with a t3Large instance, with 16GB RAM. Then allow SSH, HTTP traffic, and HTTPS traffic if you plan to build on these examples.) We have two repos:

1. The sample application

2. The configuration files

The configuration files install both nginx and the sample app. 

After you spin up an instance, install nix for all users:

```sh
sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon
```

Next, log out and log back in so that nix is available in the system path.

And then you can run System Manager and deploy the app with one command:

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake github:frecklefacelabs/system-manager-custom-app-deploy/v1.0.0#default --sudo
```

(Remember, the first time System Manager runs, it takes up to five minutes or so to compile everything.)

!!! Tip
    We're specifying a tag in our URL. This is good practice to make sure you get the right version of your flakes. Also, modern Nix supports the use of a protocol called "github", and when you use that protocol, you can specify the tag behind a slash symbol, as we did here for tag v1.0.0.

!!! Tip
    If you make changes to your flakes, be sure to create a new tag. Without it, Nix sometimes refuses to load the "latest version" of the repo, and will insist on using whatever version of your repo it used first.

Then, the app should be installed, with nginx sitting in front of it, and you should be able to run:

```sh
curl localhost
```
And it will print out a friendly JSON message such as:

```json
{"message":"Welcome to the Bun API!","status":"running","endpoints":["/","/health","/random","/cowsay"]}
```

We even included cowsay in this sample, which you can try at `curl localhost/cowsay`. Now even though cowsay is meant for fun, the primary reason is this is a TypeScript app that uses `bun`, and we wanted to demonstrate how easy it is to include `npm` libraries. `bun` includes a feature whereby it will install dependency packages from `package.json` automatically the first time it runs, greatly simplifying the setup.

One thing about the .nix files in this repo is that they in turn pull code (our TypeScript app) from another remote repo. Using this approach, you can separate concerns, placing the deployment .nix files in one repo, and the source app in a separate repo.

Here are further details on the individual nix files.

First we have a flake much like the usual starting point:

```nix
# flake.nix
{
  description = "Standalone System Manager configuration";

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
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {

        # Specify your system configuration modules here, for example,
        # the path to your system.nix.
        modules = [

          {
            nix.settings.experimental-features = "nix-command flakes";
            services.myapp.enable = true;
          }
            ./system.nix
            ./nginx.nix
            ./bun-app.nix
        ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

Next is the .nix configuration that installs and configures nginx. This is a simple ngnix configuration, as it simply routes incoming HTTP traffic directly to the app:

```nix
# nginx.nix
{ config, lib, pkgs, ... }:
{
  config = {
    services.nginx = {
      enable = true;

      recommendedGzipSettings = true;
      recommendedOptimisation = true;
      recommendedProxySettings = true;
      recommendedTlsSettings = true;

      virtualHosts."_" = {
        default = true;

        locations."/" = {
          proxyPass = "http://127.0.0.1:3000";
          extraConfig = ''
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
          '';
        };

        locations."/health" = {
          proxyPass = "http://127.0.0.1:3000/health";
          extraConfig = ''
            access_log off;
          '';
        };
      };
    };
  };
}
```

Next, here's the .nix configuration that creates a service that runs the app.

```nix
# bun-app.nix
{ config, lib, pkgs, ... }:
let
  # Fetch the app from GitHub
  appSource = pkgs.fetchFromGitHub {
    owner = "frecklefacelabs";
    repo = "typescript_app_for_system_manager";
    rev = "v1.0.0";  # Use a tag
    sha256 = "sha256-TWt/Y2B7cGxjB9pxMOApt83P29uiCBv5nVT3KyycYEA=";
  };
in
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Install Bun
    environment.systemPackages = with pkgs; [
      bun
    ];

    # Simple systemd service - runs Bun directly from Nix store!
    systemd.services.bunapp = {
      description = "Bun TypeScript Application";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        User = "ubuntu";
        Group = "ubuntu";
        WorkingDirectory = "${appSource}";
        # Bun will auto-install dependencies from package.json on first run
        ExecStart = "${pkgs.bun}/bin/bun run index.ts";
        Restart = "always";
        RestartSec = "10s";
      };

      environment = {
        NODE_ENV = "production";
      };
    };
  };
}

```

And finally, here's the `index.ts` file; it's just a simple REST app that also makes use of one third-party `npm` library.

```typescript
import cowsay from "cowsay";

const messages = [
  "Hello from System Manager!",
  "Bun is blazingly fast! ?",
  "Nix + Bun = Easy deployments",
  "Making it happen!",
  "Nix rocks!"
];

const server = Bun.serve({
  port: 3000,
  fetch(req) {
    const url = new URL(req.url);
    
    if (url.pathname === "/") {
      return new Response(JSON.stringify({
        message: "Welcome to the Bun API!",
        status: "running",
        endpoints: ["/", "/health", "/random", "/cowsay"]
      }), {
        headers: { "Content-Type": "application/json" }
      });
    }
    
    if (url.pathname === "/health") {
      return new Response(JSON.stringify({
        status: "healthy"
      }), {
        headers: { "Content-Type": "application/json" }
      });
    }
    
    if (url.pathname === "/random") {
      const randomMessage = messages[Math.floor(Math.random() * messages.length)];
      return new Response(JSON.stringify({
        message: randomMessage,
        timestamp: new Date().toISOString()
      }), {
        headers: { "Content-Type": "application/json" }
      });
    }
    
    if (url.pathname === "/cowsay") {
      const cow = cowsay.say({ 
        text: "Deployed with System Manager and Nix!" 
      });
      return new Response(cow, {
        headers: { "Content-Type": "text/plain" }
      });
    }
    
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`? Server running on http://localhost:${server.port}`);
```


# Optional: Installing System Manager Locally

Nix allows you to run code that's stored remotely in a repo, such as in GitHub. As such, you don't have to install System Manager locally to use it. However, if you want to install locally, you can do so with the following `nix profile` command.

```sh
nix profile add 'github:numtide/system-manager'
```

Or, if you don't have the optional features set in `/opt/nix/nix.conf`, you can provide them through the command line:

```sh
nix profile add 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes'
```

!!! Tip
    After System Manager is installed locally, you no longer need to worry about whether you have experimental features installed. You will simply pass the --flake option to System Manager.

When you install System Manager, you might get some warnings about trusted user; this simply means you're not in the trusted user list of nix. But System Manager will still install and work fine.

Then you can find System Manager:

```console
$ which system-manager
/home/ubuntu/.nix-profile/bin/system-manager
```

And you can run System Manager:

```sh
system-manager switch --flake . --sudo
```


!!! Tip
    System Manager is still in an early state and undergoing active development. Installing locally will not immediately pick up new changes. If you decide to install locally, you'll want to periodically check our GitHub repo for changes, and re-install it if necessary by using `nix profile upgrade`.


# More stuff, possibly:

Inspecting /var/lib/system-manager/state/system-manager-state.json

Troubleshooting Guide

Recipes (individual software packages, etc.)

Package overlays

Managing a Remote System with System Manager

Working with Timers

Managing Users

