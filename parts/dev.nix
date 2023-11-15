{inputs, ...}: {
  perSystem = {system, ...}: let
    pkgs = import inputs.nixpkgs {
      inherit system;
      overlays = [(import inputs.rust-overlay)];
    };
  in {
    devShells.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        (rust-bin.stable.latest.default.override {
          extensions = ["rust-analyzer" "rust-src"];
        })

        openssl
        pkg-config
      ];
    };

    formatter = pkgs.alejandra;
  };
}
