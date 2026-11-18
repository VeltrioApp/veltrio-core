# Patched dependencies

## librqbit-dualstack-sockets 0.7.0

Upstream binds a socket to an interface by index on macOS and by name
everywhere else except Windows. iOS and tvOS took the "everywhere else"
branch, which calls `socket2::Socket::bind_device` — not available on
Apple systems — so the crate didn't compile for them.

The only change is in `src/bind_device.rs`: the macOS branch now covers
every Apple OS. Drop this patch (and the `[patch.crates-io]` entry in
`Cargo.toml`) once upstream ships the same fix.
