{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      flake-parts,
      ...
    }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      flake.nixosModules.default = import ./parts/module.nix self;

      perSystem =
        {
          pkgs,
          self',
          ...
        }:
        {
          packages.default = pkgs.callPackage ./parts/derivation.nix { inherit self; };

          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              clippy
              (rustfmt.override { asNightly = true; })
              rust-analyzer

              redis
              xh
            ];

            inputsFrom = [ self'.packages.default ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };

          formatter = pkgs.nixfmt-tree;
        };
    };
}
