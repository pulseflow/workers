{
  description = "a collection of core workers for pulseflow";

  # todo: switch to =+ uninix
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust.url = "github:oxalica/rust-overlay";
  };

  outputs = inputs @ { self, nixpkgs, parts, crane, rust, ... }:
    parts.lib.mkFlake { inherit inputs; } {
      systems = [ "aarch64-linux" "x86_64-linux" "aarch64-darwin" ];
      perSystem = { self', lib, system, ... }:
        let
          pkgs = nixpkgs.legacyPackages.${system}.extend rust.overlays.default;
          rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = crane.lib.${system}.overrideToolchain rust-toolchain;

          craneArgs = {
            pname = "pulseflow-workers";
            version = self.rev or "dirty";
            src = craneLib.cleanCargoSource ./.;
          };

          cargoArtifacts = craneLib.buildDepsOnly craneArgs;
          pulseflow-workers = craneLib.buildPackage (craneArgs // { inherit cargoArtifacts; });
        in
        {
          apps.pulseflow-workers = {
            type = "app";
            program = lib.getExe self'.packages.default;
          };

          devShell = craneLib.devShell {
            shellHook = ''
            '';

            packages = [
              pkgs.wasm-pack
              pkgs.wrangler
              pkgs.git
              pkgs.nodejs-17_x
            ];
          };

          checks.pulseflow-workers = pulseflow-workers;
          packages.default = pulseflow-workers;
        };
    };
}
