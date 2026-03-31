{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      flake-parts,
      nixpkgs,
    }@inputs:
    let
      inherit (nixpkgs) lib;
    in
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      flake.nixosModules.default = lib.modules.importApply ./nix/module.nix { inherit self; };

      perSystem =
        {
          pkgs,
          self',
          ...
        }:
        {
          packages.default = pkgs.callPackage ./nix/package.nix { inherit self; };

          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              clippy
              (rustfmt.override { asNightly = true; })
              rust-analyzer

              valkey
              xh
            ];

            inputsFrom = [ self'.packages.default ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };

          formatter = pkgs.nixfmt-tree;
        };
    };
}
