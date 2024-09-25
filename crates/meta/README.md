# Pulseflow Minecraft Metadata Collector

A docker-compatible server that can be run as a GitHub actions cronjob that manages, formats, and produces Minecraft loader metadata that is compatible with the [`interpulse`](https://crates.io/crates/interpulse) Minecraft module specification.

- [`src/main.rs`](./src/main.rs): initializes the collection
- [`src/api/`](./src/api/): core interfaces for minecraft, fabric-based, and forge-based loader apis
  - [`src/api/fabric.rs`](./src/api/fabric.rs): manages fabric data fetching for quiltmc and fabricmc
  - [`src/api/forge.rs`](./src/api/forge.rs): manages forge data fetching for lexforge and neoforge
  - [`src/api/minecraft.rs`](./src/api/minecraft.rs): manages minecraft data fetching for mojangs apis
- [`src/utils/`](./src/utils/): core utilities for managing the database, downloading, and maven
