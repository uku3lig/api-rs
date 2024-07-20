{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    flake-parts,
    ...
  } @ inputs:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];

      flake.nixosModules.default = import ./parts/module.nix self;

      perSystem = {
        lib,
        pkgs,
        self',
        ...
      }: {
        packages.default = pkgs.callPackage ./parts/derivation.nix {inherit self;};

        devShells.default = with pkgs; mkShell {
          packages = [clippy rustfmt rust-analyzer redis];
          inputsFrom = [self'.packages.default];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };

        formatter = pkgs.alejandra;
      };
    };
}
