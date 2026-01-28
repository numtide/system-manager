# Examples

Complete, working examples demonstrating System Manager configurations for common use cases.

## Available Examples

### [Users and Groups](users.md)

Declaratively manage user accounts and groups, including normal users, system users for services, and group membership.

### [Timer](timer.md)

Create a simple systemd timer that runs every minute, demonstrating how to set up scheduled tasks.

### [Docker](docker.md)

Install Docker and configure it as a systemd service with proper socket activation and daemon configuration.

### [PostgreSQL](postgresql.md)

Set up a PostgreSQL database server with automatic initialization, user creation, and proper systemd integration.

### [Nginx](nginx.md)

Configure Nginx as a web server with HTTP support, including systemd service management and `/etc` configuration.

### [Nginx HTTPS](nginx-https.md)

Extend the Nginx configuration with SSL/TLS certificates for HTTPS support, including certificate management and security best practices.

### [Custom App](custom-app.md)

Deploy a custom TypeScript/Bun application behind Nginx, demonstrating how to fetch code from GitHub and run it as a systemd service. Includes a live example you can try.
