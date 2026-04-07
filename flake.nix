{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    inputs@{ self, nixpkgs, ... }:
    let
      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];

      forAllSystems =
        f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in
    {
      devShells = forAllSystems (
        { pkgs, ... }:
        {
          default =
            with pkgs;
            mkShell {
              packages = [
                nix
                rustc
                cargo
                mdbook
              ];
            };
        }
      );
      formatter = forAllSystems ({ pkgs, ... }: pkgs.nixfmt-tree);
      packages = forAllSystems (
        { pkgs, ... }:
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "mdbook-typst";
            version = "0.1.0";
            src = ./mdbook-typst;
            cargoLock.lockFile = ./mdbook-typst/Cargo.lock;
          };
        }
      );
    };
}
