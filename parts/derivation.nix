{
  lib,
  rustPlatform,
  mold-wrapped,
  self,
}:
rustPlatform.buildRustPackage {
  pname = "api-rs";
  version = self.shortRev or self.dirtyShortRev or "dirty";

  src = self;

  nativeBuildInputs = [mold-wrapped];

  cargoLock.lockFile = ../Cargo.lock;

  RUSTFLAGS = "-C link-arg=-fuse-ld=mold";

  doCheck = false;

  meta = with lib; {
    mainProgram = "api-rs";
    description = "backend for api.uku3lig.net";
    homepage = "https://github.com/uku3lig/api-rs";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}
