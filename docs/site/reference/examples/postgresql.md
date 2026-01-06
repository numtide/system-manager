# PostgreSQL

This example shows how to install and configure PostgreSQL as a systemd service.

## Prerequisites

System Manager is still in its early state, and doesn't yet have user management, which is a planned feature that will be here soon. As such, for now, before you run this, you'll need to manually create the postgres user. Additionally, go ahead and create two directories and grant the postgres user access to them:

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

## Configuration

Here's the `.nix` file that installs PostgreSQL.

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

## What this configuration does

1. **Installs PostgreSQL 16** as a system package
2. **Creates a systemd service** that:
   - Runs as the `postgres` user
   - Initializes the database directory on first run
   - Starts PostgreSQL with the data directory at `/var/lib/postgresql/16`
3. **Creates an initialization service** that:
   - Waits for PostgreSQL to be ready
   - Creates a database called `myapp`
   - Creates a user called `myapp`
   - Grants appropriate privileges
