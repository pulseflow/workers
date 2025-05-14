# workers

a collection of core workers for pulseflow apis

interpulse forked from `modrinth/daedalus` under MIT license.

## MSRV (Minimum Supported Rust Version)

the current MSRV is `1.80.0`, but the latest `nightly` version is recommended.
this is because the metadata server used `LazyLock` which was introduced in `1.80.0`.
if you are exclusively using `interpulse`, the MSRV is lower.
