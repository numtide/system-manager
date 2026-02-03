# PostgreSQL

This example shows how to install and configure PostgreSQL as a systemd service.

## Configuration

Here's the `.nix` file that installs PostgreSQL with declarative user management.

```nix
{ config, lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Create the postgres system user and group
    users.users.postgres = {
      isSystemUser = true;
      group = "postgres";
      home = "/var/lib/postgresql";
      createHome = true;
      description = "PostgreSQL server";
    };

    users.groups.postgres = {};

    # Create the runtime directory for PostgreSQL socket
    systemd.tmpfiles.rules = [
      "d /run/postgresql 0755 postgres postgres -"
    ];

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

## What this configuration does

1. **Creates the postgres user and group** declaratively via `users.users` and `users.groups`
2. **Creates the runtime directory** `/run/postgresql` via tmpfiles for the PostgreSQL socket
3. **Installs PostgreSQL 16** as a system package
4. **Creates a systemd service** that:
   - Runs as the `postgres` user
   - Initializes the database directory on first run
   - Starts PostgreSQL with the data directory at `/var/lib/postgresql/16`
5. **Creates an initialization service** that:
   - Waits for PostgreSQL to be ready
   - Creates a database called `myapp`
   - Creates a database user called `myapp`
   - Grants appropriate privileges
