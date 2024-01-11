{inputs, ...}: {
  perSystem = {lib, system, ...}: let
    pkgs = import inputs.nixpkgs {
      inherit system;
      overlays = [(import inputs.rust-overlay)];
    };
  in {
    devShells.default = with pkgs; mkShell {
      buildInputs = [
        (rust-bin.stable.latest.default.override {
          extensions = ["rust-analyzer" "rust-src"];
        })

        openssl
      ];

      nativeBuildInputs = [pkg-config];
      packages = [nil];

      LD_LIBRARY_PATH = lib.makeLibraryPath [openssl];
    };

    formatter = pkgs.alejandra;
  };
}
