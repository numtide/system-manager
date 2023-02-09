
## Profile generation


## Activation strategy
The activation script calls `system-manager activate`,
which will perform the following actions.

### Systemd services
The info about services (name and store path of the service file) is found
in a file called `services/services.json` in the system-manager configuration directory.
The info about the services that were part of the previous generation is stored
in a state file at `/var/lib/system-manager`.
We then:
1. Compare the list of services present in the current configuration with the
   ones stored in the state file from the previous generation.
1. For all services in the new generation,
   create a symlink from `/etc/systemd/system/<service name>` to the service file
   in the nix store.
1. For all services present in the old generation but not in the new one:
   1. Stop the service.
   1. Remove the symlink from `/etc/systemd/system`.
1. Perform a systemd daemon-reload
1. Start the services that are present in this generation and not in the previous one
1. Restart services that are present in both

This approach basically ignores the `wantedBy` option.
A future version might improve upon this, but one of the complexities is that
NixOS does not encode the `wantedBy` option in the generated unit files, but
rather produces `<unit name>.wants` directories in the directory that
`/etc/systemd/system` gets linked to.
Supporting this properly would mean that we need to find a way to register
the `wantedBy` option on a non-NixOS system in a way such that we can use it.

### Udev rules


### Files under `/etc`
