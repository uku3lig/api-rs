{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
  self,
}:
rustPlatform.buildRustPackage {
  pname = "api-rs";
  version = self.shortRev or self.dirtyShortRev or "dirty";

  cargoLock.lockFile = ../Cargo.lock;

  src = self;

  buildInputs = [openssl];
  nativeBuildInputs = [pkg-config];

  meta = with lib; {
    mainProgram = "api-rs";
    description = "backend for api.uku3lig.net";
    homepage = "https://github.com/uku3lig/api-rs";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}
