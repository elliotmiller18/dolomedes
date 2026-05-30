{
  description = "Nix development environment for dolomedes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [
            "clippy"
            "rust-analyzer"
            "rust-src"
            "rustfmt"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo-nextest
            pkg-config
            rustToolchain
          ];

          buildInputs = with pkgs;
            lib.optionals stdenv.isDarwin [
              libiconv
            ];

          shellHook = ''
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
          '';
        };
      }
    );
}
