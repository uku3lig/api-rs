{
  inputs = {
    nixpkgs.url = "nixpkgs";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    flake-parts,
    ...
  } @ inputs:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];

      imports = [
        ./parts/dev.nix
      ];

      flake.nixosModules.default = import ./parts/module.nix self;

      perSystem = {
        pkgs,
        self',
        ...
      }: {
        packages = {
          api-rs = pkgs.callPackage ./parts/derivation.nix {inherit self;};
          api-rs-smol = self'.packages.api-rs.override {optimizeSize = true;};
          default = self'.packages.api-rs;
        };
      };
    };
}
