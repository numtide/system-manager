# Getting Started

If you’ve heard of NixOS, you’ve probably heard that it lets you define your entire system in configuration files and then reproduce that system anywhere with a single command. System Manager brings that same declarative model to any Linux distribution, with no reinstalling, no switching operating systems, and no special prerequisites beyond having Nix installed. Instead of manually installing packages, editing /etc files, or configuring system services by hand, you describe the desired state of your machine in a small set of Nix files. System Manager reads that configuration, applies it safely, and keeps previous versions so you can roll back at any time. This guide introduces those ideas step by step, helping you gain the benefits of Nix-style reproducibility and consistency on the Linux system you already have.

# Initializing Your System

To get started with System Manager, you can run our init subcommand, which will create an initial set of files in the `~/.config/system-manager` folder. In a shell prompt, enter the following:

```
nix run 'github:numtide/system-manager' -- init
```

(Remember, the double dash -- signifies that any options following it are passed to the following command, in this case system manager, rather than to the main command, `nix`).

This will create some files in the ~/.config/system-manager folder:




# How can I tell whether Nix is installed for the whole system or just me?

Simply type

```
which nix
```

If you see it's installed off of your home directory, e.g.:

```
/home/username/.nix-profile/bin/nix
```

Then it's installed just for you. Alternatively, if it's installed for everybody, it will be installed like so:

```
/nix/var/nix/profiles/default/bin/nix
```

# Example: Installing/Uninstalling Apps

[Similar to README]

# Example: Creating a System Service

[Move from README]

# Example: Saving a file to the /etc folder

[Move from README]


# Storing your files on a GitHub repo

Another option is to store your files in a remote repo (typically GitHub) and access them remotely without even saving them locally.

To do this, you need to make sure you have an updated flake.lock file. Then you can simply point System Manager to the remote repo:




## Understanding Imperative State vs Declarative State

Things that are defined delaratively in a configuration file and applied automatically should always produce the same state.

Examples of imperative state:

* Editing /etc/ssh/sshd_config directly

* Running apt install <package>

* Manually creating /etc/profile.d/foo.sh

* Toggling systemd units with systemctl enable

* Changing permissions or ownership manually



Comparison:

Imperative: telling the computer how to do something, step by step.

Declarative: telling the computer what the end state should be, and letting the system figure out how to achieve it.

