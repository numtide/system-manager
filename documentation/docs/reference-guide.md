# Reference Guide

[This might be starting to get a little long. We might want to split it up into separate guides. We could move the "Setting up a folder and file structure" section to the Getting Started Guide?]

To get the most out of System Manager, we're offering this guide to help you make the best decisions based on your particular situation.

## Table of Contents

1. [System Requirements](#system-requirements)

2. [Optional: Installing Locally](#optional-installing-locally)

3. [Running under sudo](#running-under-sudo)

4. [Setting up a folder and file structure](#setting-up-a-folder-and-file-structure)

5. [Letting System Manager manage /etc/nix/nix.conf](#letting-system-manager-manage-etcnixnixconf)

6. [Building system-manager .nix files](#building-system-manager-nix-files)

7. [Managing System Services](#managing-system-services)

8. [Managing Software Installations](#managing-software-installations)

9. [Working With /etc Files Declaratively](#working-with-etc-files-declaratively)

10. [Working with Timers](#working-with-timers)

11. [Dealing with conflicting files](#dealing-with-conflicting-files)

12. [Working with remote flakes](#working-with-remote-flakes)

13. [Managing a Remote System with System Manager](#managing-a-remote-system-with-system-manager)

14. [Command-line Options](#command-line-options)

15. [Full Examples](#full-examples)

    a. [Installing PostgreSQL](#full-example-installing-postgresql)

    b. [Installing Nginx](#full-example-installing-nginx)

    c. [Managing a System that runs Custom Software](#full-example-managing-a-system-that-runs-custom-software)

    d. [Managing a System that runs Custom Software along with an SSL Certificate](#full-example-managing-a-system-that-runs-custom-software-along-with-an-ssl-certificate)


16. FAQ (Maybe put in its own document)

# System Requirements

[TODO: I realized I wrote this section twice, here and in the getting-started guide. I'll combine them and provide the same in both, since some people might just read one doc or the other.]

[TODO: I tried 8GB and that wasn't enough to run System Manager. I tried 16GB, and that worked. I'll do some more testing to see if I can find the lowest number.]

In order to use System Manager, you need:

* **Nix installed system-wide.** (System Manager doesn't work with a per-user installation of Nix)
* **A Linux machine.** We've tested System Manager with Ubuntu both as standalone and under Windows Subsystem for Linux (WSL).
* **At least 16GB Memory.** (This is primarily due to Nix; if you're using System Manager to configure, for example, small servers on the Cloud, 8GB won't be enough.)

## Optional: Installing Locally

Nix allows you to run code that's stored remotely in a repo, such as in GitHub. As such, you don't have to install System Manager locally to use it. However, if you want to install locally, you can do so with the following `nix profile` command.

```
nix profile add 'github:numtide/system-manager'
```

When you install System Manager, you might get some warnings about trusted user; this simply means you're not in the trusted user list of nix. But System Manager will still install.

Then you can find it:

```
$ which system-manager
/home/ubuntu/.nix-profile/bin/system-manager
```

[Might be nice to list some ideas about When would you install locally... I'm not sure, can anyone help?]

!!! Note
    Throughout this Guide, we do not use the local installation of System Manager; instead, we run it directly from our GitHub repository.

!!! Tip
    System Manager is still in an early state and undergoing active development. Installing locally will not immediately pick up new changes. If you decide to install locally, you'll want to periodically check our GitHub repo for changes, and re-install it if necessary by using `nix profile upgrade`.

## Running under sudo

System Manager needs `sudo` access to run. However, by default, Linux limits the $PATH variable when running under `sudo`. That means when you run:

```
sudo nix --version
```

you'll likely get

```
sudo: nix: command not found
```

There are many workarounds here, some of which present a bit of danger. For example, you might consider modifying the secure_path setting in the /etc/sudoers file; however, you might reconsider. A lot can go wrong:

* Changes affect all sudo users
* Scripts that rely on the standard secure_path could break
* Other admins might not be aware of the change; or if they find it, they might reset it and break the way you're configuring the system
* Adding paths to secure_path can result in escalation attacks

And so on; there's a full laundry list of reasons not to do it.

Instead, we recommend simply passing the path to the sudo environment; this is temporary for the duration of the command. Here's the general format:

```
sudo env PATH="$PATH" nix --version
```

This should print out the nix version, similar to this:

```
nix (Nix) 2.32.4
```

!!! Note
    Adding yourself to Nix's trusted-users configuration won't help here. Trusted users have elevated privileges within the Nix daemon, but System Manager requires root filesystem permissions to modify /etc, manage services, and install system packages. You'll still need to use sudo.

# Setting up a folder and file structure

[TEAM: SHOULD THIS GO HERE OR IN THE GETTING STARTED GUIDE? I DON'T WANT TO WEIGH DOWN THE GETTING STARTED GUIDE TOO MUCH, BUT MAYBE IT STILL BELONGS THERE? --JEFFREY]

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

# Letting System Manager manage `/etc/nix/nix.conf`

System Manager can optionally manage your `/etc/nix/nix.conf` file for you.

If you have an existing `/etc/nix/nix.conf` file, it will be overwritten and your changes will be lost. Further, from that point on System Manager will manage the file for you, and you should not make changes to it.

Instead, you'll put the changes in one of your .nix files you'll be building to configure System Manager.

The example in the next section demonstrates how to do this.

# Building system-manager .nix files

Ready for an example! For this example, we're going to use the following:

* Our files will live in `~/.config/system-manager`

* We'll have two files, one `flake.nix`, and `system.nix`

Note that we'll be using the files generated by the System Manager's `init` subcommand. But to show that we're not locked into that format, later we'll demonstrate a single flake.nix file. Then in the sections that follow, we'll demonstrate how you can further split up your files.

We'll demonstrate how to install an app on your machine, then we'll add another app, then we'll uninstall the first app.

We'll also demonstrate how to move items from your `/etc/nix/nix.conf` file into your System Manager configuration file.

## .nix configuration setup

In your configuration .nix file, you'll typically set up everything under the config object. Inside this object you'll want some or all of the following:

* nixpkgs.hostPlatform: This specifies the platform such as nixpkgs.hostPlatform = "x86_64-linux";
* services
* environment, consisting of
  * systemPackages
  * etc
* systemd.services
* systemd.tmpfiles


[Coming soon: A single flake.nix file]

# Managing System Services

System Manager lets you manage systemd services declaratively, using the same module language you used for installing packages or creating files under /etc. Instead of manually placing service files in /etc/systemd/system or enabling them with systemctl, you describe the service in a Nix module—its command, environment, dependencies, restart behavior, and any timers or sockets it needs.

System Manager then generates the correct systemd unit files, installs them into the right directory, and reloads systemd automatically during a switch. This approach gives you repeatability and safety: if you rebuild a machine, the same services come back exactly as before; if a service configuration breaks, you simply roll back to the previous generation. Declarative service management also avoids drift—no accidental edits, no forgotten manual steps, and no inconsistencies between machines or team members.

Using this approach, instead of manually saving a file in `/etc/systemd/system` and then manually starting and stopping the service, you use a `.nix` file to *declaratively* state what you want the service to look like, and that you want it to be active.

Then you can take this same `.nix` file, place it on another system, and run System Manager again, and you'll have the service installed in a way that's identical to the first system.


The following example demonstrates how to specify a system service and activate it.

Update your flake.nix file to include a new file in the modules list, which we'll call `say_hello.nix`:

```nix
        modules = [
            ./system.nix
            ./apps.nix
            ./say_hello.nix
        ];
```

Then create the file called `say_hello.nix` and add the following to it:

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

```
wantedBy = [ "system-manager.target" ];
```

(There are other options for wantedBy; we discuss it in full in our Reference Guide under [Specifying WantedBy Setting](./reference-guide.md#specifying-the-wantedby-setting))

Activate it using the same nix command as earlier:

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

This will create a system service called `say-hello` (the name comes from the line `config.systemd.services.say-hello`) in a unit file at `/etc/systemd/system/say-hello.service` with the following inside it:

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
> Compare the lines in the `say-hello.service` file with the `say_hello.nix` file to see where each comes from.

You can verify that it ran by running journalctl:

```
journalctl -n 20
```

and you can find the following output in it:

```
Nov 18 12:12:51 my-ubuntu systemd[1]: Starting say-hello.service - say-hello...
Nov 18 12:12:51 my-ubuntu say-hello-start[3488278]: Hello, world!
Nov 18 12:12:51 my-ubuntu systemd[1]: Finished say-hello.service - say-hello.
```

> [!Note]
> If you remove the `./apps.nix` line from the `flake.nix`, System Manager will see that the configuration changed and that the apps listed in it are no longer in the configuration. As such, it will uninstall them. This is normal and expected behavior.


## Specifying the wantedBy Setting

The wantedBy attribute tells systemd when to automatically start a service. System Manager includes its own systemd target that you can use in the wantedBy setting to automatically start any services immediately after applying the changes, as well as after reboot. Here's an example wantedBy line in a .nix configuration file:

```
wantedBy = [ "system-manager.target" ];
```

(By allowing the service to start after applying changes, you don't need to reboot for the service to start.)

But you're not limited to just this target. For example, if you're creating a system service that runs on a schedule, you might use this:

```
wantedBy = [ "timers.target" ]
```

# Managing Software Installations

System Manager allows you to install software in a fully declarative way similar to installing system services. Instead of relying on a traditional package manager and running commands like apt install or dnf install, you list the packages you want in your configuration file. During a switch, System Manager builds a new system profile that includes those packages, activates it, and ensures the software is available on your PATH. This makes installations reproducible and version-controlled. If you reinstall your operating system or set up a new machine, the exact same tools will appear automatically. And because software installation is tied to your configuration (not to manual actions), System Manager prevents drift—no forgotten tools, no mismatched versions across machines, and no surprises when you rollback or update.

Oftentimes, when you're creating a system service, you need to create a configuration file in the `/etc` directory that accompanies the service. System manager allows you to do that as well.

Add another line to your `flake.nix` file, this time for `./sample_etc.nix`:

```nix
        modules = [
            ./system.nix
            ./apps.nix
            ./say_hello.nix
            ./sample_etc.nix
        ];
```

Then, create the `sample_etc.nix` file with the following into it:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    
    environment.etc = {
      sample_configuration = {
        text = ''
          This is some sample configuration text
        '';
      };
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

# Working With /etc Files Declaratively

Many applications and services rely on configuration files stored under /etc, and System Manager lets you manage those files declaratively as well. Instead of manually editing files like /etc/some_config, you define them in your Nix configuration and let System Manager write them during a switch. This ensures that your system state is always consistent with your configuration and avoids accidental edits or configuration drift. If you ever rebuild your machine, those files are recreated exactly as before—permissions, contents, and paths included. And because System Manager keeps previous generations, you can safely roll back to earlier versions of /etc files if needed. Declarative /etc management is especially powerful in shared or multi-machine environments, where consistency and repeatability matter most.

[Examples next]

[Managing permissions]

[Overriding package versions]

[System manage and symlinks]

# Working with Timers

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

```b
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake git+https://github.com/frecklefacelabs/system-manager-test#default
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


# Managing a Remote System with System Manager

[Coming soon]

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

[What about the result/bin folder? I see activate  deactivate  preActivationAssertions  prepopulate  register-profile]

# Full Examples

## Full Example: Installing PostgreSQL

[Let's put this in a github repo and in its own .nix file making the installation of postgres a snap for everyone reading this from now on...]

Note: System Manager is still in its early state, and doesn't yet hae user management. As such, before you run this, you'll need to manually create the postgres user. Additionally, go ahead and create two directories and grant the postgres user access to them:

```bash
# Create postgres user and group
sudo groupadd -r postgres
sudo useradd -r -g postgres -d /var/lib/postgresql -s /bin/bash postgres

# Create directories with proper permissions
sudo mkdir -p /var/lib/postgresql
sudo chown postgres:postgres /var/lib/postgresql

sudo mkdir -p /run/postgresql
sudo chown postgres:postgres /run/postgresql
```

Here's a flake that installs PostgreSQL.

[More docs shortly; this is fully tested.]

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

[docs coming soon, the following nix file is tested and works on a clean system... using the init's system.nix, but can also show as standalone]

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

## Full Example: Managing a System that runs Custom Software

Here's an example where you might have custom web software living in a repository and you want to run the software on a system behind nginx.

## Live example

We have a complete example live that you can try out. All you need is a fresh server (such as on Amazon EC2) with at least 16GB memory. (We recommend the latest Ubuntu, with a t3Large instance, with 16GB RAM. Then allow SSH, HTTP traffic, and HTTPS traffic if you plan to build on these examples.) We have two repos:

1. The sample application

2. The configuration files

The configuration files install both nginx and the sample app. 

After you spin up an instance, install nix for all users:

```
sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon
```

Next, log out and log back in so that nix is available in the system path.

And then you can run System Manager and deploy the app with one command:

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake github:frecklefacelabs/system-manager-custom-app-deploy/v1.0.0#default
```

(Remember, the first time System Manager runs, it takes up to five minutes or so to compile everything.)

!!! Tip
    We're specifying a tag in our URL. This is good practice to make sure you get the right version of your flakes. Also, modern Nix supports the use of a protocol called "github", and when you use that protocol, you can specify the tag behind a slash symbol, as we did here for tag v1.0.0.

!!! Tip
    If you make changes to your flakes, be sure to create a new tag. Without it, Nix sometimes refuses to load the "latest version" of the repo, and will insist on using whatever version of your repo it used first.

Then, the app should be installed, with nginx sitting in front of it, and you should be able to run:

```
curl localhost
```
And it will print out a friendly JSON message such as:

```
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

```
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

```
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

## Full Example: Managing a System that runs Custom Software along with an SSL Certificate

[Coming soon; this is particular important, as the previous example only does HTTP, not HTTPS]

# More stuff, possibly:

Inspecting /var/lib/system-manager/state/system-manager-state.json

Adding in Blueprint

Security issues -- sudo, trusted users, etc.

Troubleshooting Guide

