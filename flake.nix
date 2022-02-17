{
  description = "A simple library to deal with maildir folders";

  inputs = {
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, utils, rust-overlay, naersk, ... }:
    utils.lib.eachDefaultSystem
      (system:
        let
          name = "maildir";
          naersk-lib = naersk.lib.${system};
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              rust-overlay.overlay
              (self: super: {
                rustc = self.rust-bin.stable.latest.default;
                cargo = self.rust-bin.stable.latest.default;
              })
            ];
          };
        in
        rec {
          # nix build
          defaultPackage = packages.${name};
          packages = {
            ${name} = naersk-lib.buildPackage {
              pname = name;
              root = ./.;
              nativeBuildInputs = with pkgs; [ openssl.dev pkgconfig ];
            };
          };

          # nix run
          defaultApp = apps.${name};
          apps.${name} = utils.lib.mkApp {
            inherit name;
            drv = packages.${name};
          };

          # nix develop
          devShell = pkgs.mkShell {
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            inputsFrom = builtins.attrValues self.packages.${system};
            buildInputs = with pkgs; [
              cargo
              cargo-watch
              trunk
              ripgrep
              rust-analyzer
              rustfmt
              rnix-lsp
              nixpkgs-fmt
            ];
          };
        }
      );
}
