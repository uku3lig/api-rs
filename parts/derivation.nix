{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
  self,
  lto ? true,
  optimizeSize ? false,
}:
rustPlatform.buildRustPackage {
  pname = "api-rs";
  version = builtins.substring 0 8 self.lastModifiedDate or "dirty";

  cargoLock.lockFile = ../Cargo.lock;

  src = self;

  buildInputs = [openssl];
  nativeBuildInputs = [pkg-config];

  RUSTFLAGS =
    lib.optionalString lto " -C lto=thin -C embed-bitcode=yes"
    + lib.optionalString optimizeSize " -C codegen-units=1 -C strip=symbols -C opt-level=z";

  meta = with lib; {
    mainProgram = "api-rs";
    description = "backend for api.uku3lig.net";
    homepage = "https://github.com/uku3lig/api-rs";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}
