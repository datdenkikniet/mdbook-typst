{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ self, nixpkgs, ... }:
    let
      # forAllSystems follows this guide:
      # https://github.com/Misterio77/nix-starter-configs/issues/64#issuecomment-1941420712
      pkgsFor =
        system:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              inputs.fenix.overlays.default
            ];
          };
          rustToolchain = pkgs.fenix.fromToolchainFile {
            dir = ./.;
            sha256 = "sha256-zC8E38iDVJ1oPIzCqTk/Ujo9+9kx9dXq7wAwPMpkpg0=";
          };
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
        in
        {
          inherit
            system
            pkgs
            rustToolchain
            rustPlatform
            ;
        };

      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (pkgsFor system));
    in
    {
      devShells = forAllSystems (
        { pkgs, rustToolchain, ... }:
        {
          default =
            with pkgs;
            mkShell {
              packages = [
                nix
                rustToolchain
                mdbook
              ];
            };
        }
      );
      formatter = forAllSystems ({ pkgs, ... }: pkgs.nixfmt-tree);
      packages = forAllSystems (
        { pkgs, rustPlatform, ... }:
        {
          default = rustPlatform.buildRustPackage {
            pname = "mdbook-typst";
            version = "0.1.0";
            src = ./mdbook-typst;
            cargoLock.lockFile = ./mdbook-typst/Cargo.lock;
          };
        }
      );
    };
}
