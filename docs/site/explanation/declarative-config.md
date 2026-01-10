# Declarative Configuration

This page explains the declarative configuration paradigm that System Manager uses, and why it matters for system administration.

## Imperative vs Declarative

### Imperative: Step-by-Step Commands

Traditional system administration is *imperative*. You tell the system *how* to do things, step by step:

```bash
apt install nginx
vim /etc/nginx/nginx.conf
systemctl enable nginx
systemctl start nginx
```

Each command mutates the system in place. Over time, you may forget what changes you made, commands may fail partway through, and two machines configured "the same way" may end up different.

### Declarative: Desired State

System Manager uses a *declarative* approach. You describe *what* the system should look like:

```nix
{
  environment.systemPackages = [ pkgs.nginx ];

  services.nginx = {
    enable = true;
    virtualHosts."example.com" = {
      root = "/var/www";
    };
  };
}
```

System Manager figures out the steps to make your system match this description. Run it again, and nothing changes (the system already matches). Run it on a fresh machine, and it applies everything needed.

## Benefits of Declarative Configuration

### Reproducibility

Your configuration files are a complete description of system state. Copy them to another machine, run System Manager, and you get an identical setup.

### No Configuration Drift

Imperative systems drift over time. Someone SSHs in, makes a quick fix, forgets to document it. With declarative configuration, if it's not in your `.nix` files, it doesn't exist.

### Version Control

Your entire system configuration is text files. Store them in Git:

- Track every change with commit history
- Review changes in pull requests
- Bisect to find when something broke
- Branch for experiments

### Safe Rollback

System Manager keeps previous configurations as "generations." If an update breaks something, rollback in seconds:

```bash
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --rollback
nix run 'github:numtide/system-manager' -- activate --sudo
```

### Team Collaboration

With configuration in Git:

- New team members can understand the system by reading code
- Changes require review before merging
- Everyone works from the same source of truth

## The Mental Shift

Moving from imperative to declarative requires a mindset change:

| Imperative Thinking | Declarative Thinking |
|---------------------|---------------------|
| "Install nginx" | "The system has nginx" |
| "Edit the config file" | "The config file contains..." |
| "Restart the service" | "The service is running" |
| "What did I change?" | "Read the config file" |

You stop thinking about *actions* and start thinking about *state*.

## When Declarative Matters Most

Declarative configuration is most valuable when:

- Managing multiple similar machines
- Onboarding new team members
- Recovering from failures
- Auditing system state
- Ensuring compliance

For a single laptop you rarely change, imperative is fine. For anything more complex, declarative pays dividends.

## See Also

- [Introduction](introduction.md) - What is System Manager?
- [How System Manager Works](how-it-works.md) - Architecture details
- [Getting Started](../tutorials/getting-started.md) - Try it yourself
