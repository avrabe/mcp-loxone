{
  description = "MCP server for Loxone home automation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    let
      # Build the package for a given system
      mkLoxoneMcp = system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };
          rustToolchain = pkgs.rust-bin.stable.latest.default;

          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
            perl
          ];

          buildInputs = with pkgs; [
            openssl
          ];
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "loxone-mcp";
          version = "0.8.0";
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          inherit nativeBuildInputs buildInputs;

          # Tests run in CI; Nix sandbox lacks network access for integration tests
          # and has url crate version conflicts in test compilation
          doCheck = false;

          meta = {
            description = "MCP server for Loxone home automation";
            homepage = "https://github.com/avrabe/mcp-loxone";
            license = with pkgs.lib.licenses; [ mit asl20 ];
          };
        };
    in
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        loxone-mcp = mkLoxoneMcp system;
      in {
        packages = {
          default = loxone-mcp;
          loxone-mcp = loxone-mcp;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkgs.rust-bin.stable.latest.default
            pkg-config
          ];
          buildInputs = with pkgs; [ openssl ];
          RUST_LOG = "debug";
        };
      }
    ) // {
      # openclawPlugin as a function of system (top-level, not per-system)
      openclawPlugin = system: {
        name = "loxone";
        skills = [ ./skills/loxone ];
        packages = [ (mkLoxoneMcp system) ];
        needs = {
          stateDirs = [ ".config/loxone-mcp" ];
          requiredEnv = [ "LOXONE_HOST" "LOXONE_USER" "LOXONE_PASS" ];
        };
      };
    };
}
