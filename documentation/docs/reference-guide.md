# Reference Guide

To get the most out of System Manager, we're offering this guide to help you make the best decisions based on your particular situation.

# Setting up a folder and file structure

Before you begin with System Manager, you'll need to decide on your folder structure.

!!! Tip
    If you prefer, you can host all your System Manager configuration files on a remote Git repo (such as GitHub), and then you don't need to worry about where on your computer to store the files.

Technically, you are free to set up your folders and files however you like; System Manager does not enforce any rules, thus allowing you full flexibility. Below are simply some options that we recommend.

!!! Tip
    While you are free to have different system manager .nix files scattered throughout your system, we recommend, if possible, keeping them in a single location simply for organizational purposes. But again, this is just a recommendation and you're not bound by any such rules.

## Deciding on a folder structure

You’ll need to choose where your System Manager configuration will live. Here are two main organizational patterns we recommend.

* **Option A**: A single folder for all your configuration (recommended)

A single folder keeps everything together. This offers a clean long-term solution, along with easy version control. It's also convenient for replicating between machines.

* **Option B**: A separate folder for each use case

While not as common, it’s entirely possible to organize your System Manager configuration into multiple independent folders, each focused on a specific use case. In this model, you treat each configuration as its own standalone unit, often stored in its own Git repository.

For example, you might keep:

* a dedicated configuration folder strictly for managing nginx,

* another for custom systemd services,

* another for developer tools,

* and yet another for monitoring and observability packages.

In this manner, you can then build up a system by picking and choosing which services you need for a particular machine, and pull each one down from GitHub.

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

# Building a first system-manager file

For this example, we're going to use the following:

* Our file will live in `~/.config/system-manager`

* We'll have two files, one flake.nix, and one default.nix

[Coming soon]



# Regarding how Nix is installed

[Coming soon, regarding single user installation vs installed for everybody]



# Managing System Services

System Manager lets you manage systemd services declaratively, using the same module language you used for installing packages or creating files under /etc. Instead of manually placing service files in /etc/systemd/system or enabling them with systemctl, you describe the service in a Nix module—its command, environment, dependencies, restart behavior, and any timers or sockets it needs. System Manager then generates the correct systemd unit files, installs them into the right directory, and reloads systemd automatically during a switch. This approach gives you repeatability and safety: if you rebuild a machine, the same services come back exactly as before; if a service configuration breaks, you simply roll back to the previous generation. Declarative service management also avoids drift—no accidental edits, no forgotten manual steps, and no inconsistencies between machines or team members.

[Examples next]

# Managing Software Installations

System Manager allows you to install software in a fully declarative way similar to installing system services. Instead of relying on a traditional package manager and running commands like apt install or dnf install, you list the packages you want in your configuration file. During a switch, System Manager builds a new system profile that includes those packages, activates it, and ensures the software is available on your PATH. This makes installations reproducible and version-controlled. If you reinstall your operating system or set up a new machine, the exact same tools will appear automatically. And because software installation is tied to your configuration (not to manual actions), System Manager prevents drift—no forgotten tools, no mismatched versions across machines, and no surprises when you rollback or update.

[Examples next]

# Working With /etc Files Declaratively

Many applications and services rely on configuration files stored under /etc, and System Manager lets you manage those files declaratively as well. Instead of manually editing files like /etc/some_config, you define them in your Nix configuration and let System Manager write them during a switch. This ensures that your system state is always consistent with your configuration and avoids accidental edits or configuration drift. If you ever rebuild your machine, those files are recreated exactly as before—permissions, contents, and paths included. And because System Manager keeps previous generations, you can safely roll back to earlier versions of /etc files if needed. Declarative /etc management is especially powerful in shared or multi-machine environments, where consistency and repeatability matter most.

[Examples next]

# Building flakes

[Coming soon]

## Regarding the config top-level object

[Coming soon; noting the differences in how to approach config and environment.systemPackages in a single file... etc...]

# Dealing with conflicting files



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

```
nix flake update
```


And make sure you've pushed it up to the repo. (If you don't do this step, nix will try to build a flake.lock, but will be unable to write it to the same location as the other files, and will error out.)


[todo: Let's create a repo under numtide for this instead of using mine --jeffrey]

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake git+https://github.com/frecklefacelabs/system-manager-test#default
```

### When should up you update your flake.nix file?

Generally, you only need to update your flake.nix file when you want newer versions of your inputs (nixpkgs, etc). Updating isn't necessary for daily use; your configuration will continue to work with the locked versions. But you will want to update your flake.lock file in cases such as:

* You want newer package versions (e.g. newer `btop`, etc.)
* You want security patches 
* You've added new imputs to your flakes (in which case you'll be required to update `flake.lock`)
* You're preparing a fresh install and decide this is a good time to upgrade everything

### Can't System Manager build flake.lock for me?

Yes, but only if the flake.nix file is local to your machine. The problem is System Manager will try to write a flake.lock file in the same location as the flake.nix file, which isn't possible (at this time) with a GitHub repo.

## Running System Manager with a remote flake

!!! Tip
    Before you run this command, we recommend that you nevertheless create a folder to run it from, such as ~/.config/system-manager. 

# Integrating Sudo with the --sudo and --sudo-ask-password flags

Normally, to make changes at the system level, System Manager requires sudo access, which requires running System Manager itself with elevated privileges (such as typing sudo at the start of the command). This can get messy, however, because by default sudo doesn't have access to a typical full path, which includes the nix command itself.

So as an alternative, we've added the --sudo and --sudo-ask-password flags to allow System Manager itself to step up with elevated privileges, simplifying the command.

[More coming soon after I've had a chance to test the feature]

# Command-line Options

## init

## switch

## register

## build

## deactivate

## pre-popular

## sudo

## ask-sudo-password

