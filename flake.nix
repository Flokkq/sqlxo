{
  description = "Basic Rust flake for magazin api";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
        with pkgs; {
          devShells.default = mkShell rec {
            buildInputs =
              [
                (rust-bin.nightly."2025-12-15".default.override {
                  extensions = ["rust-src" "rust-analyzer"];
                })

                cargo-watch
                cargo-tarpaulin
                cargo-deny
                cargo-edit
                clippy
                typos
                git-cliff

                # dev
                pkg-config
                openssl
                cacert
                sqlx-cli
                postgresql_16
                dotenvx
              ]
              ++ pkgs.lib.optionals pkg.stdenv.isDarwin [
                darwin.apple_sdk.frameworks.SystemConfiguration
              ];

            LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
            REQWEST_USE_RUSTLS = 1;
            CURL_SSL_BACKEND = "secure_transport";
            SWAGGER_UI_SKIP_SSL_CHECK = 1;
          };
        }
    );
}
