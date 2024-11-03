{
  lib,
  rustPlatform,
  mold-wrapped,
  self,
}: let
  cargoToml = lib.importTOML ../Cargo.toml;
  rev = self.shortRev or self.dirtyShortRev or "dirty";
in
  rustPlatform.buildRustPackage {
    pname = cargoToml.package.name;
    version = "${cargoToml.package.version}+git.${rev}";

    src = self;

    nativeBuildInputs = [mold-wrapped];

    cargoLock.lockFile = ../Cargo.lock;

    RUSTFLAGS = "-C link-arg=-fuse-ld=mold";

    doCheck = false;

    meta = with lib; {
      mainProgram = cargoToml.package.name;
      description = "backend for api.uku3lig.net";
      homepage = "https://github.com/uku3lig/api-rs";
      license = licenses.mit;
      platforms = platforms.unix;
    };
  }
