{
  lib,
  rustPlatform,
  installShellFiles,
}:

rustPlatform.buildRustPackage {
  pname = "colorctl";
  version = "0.1.2";

  src = lib.cleanSource ./..;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [ installShellFiles ];

  postInstall = ''
    installShellCompletion --cmd colorctl \
      --bash <($out/bin/colorctl completions bash) \
      --fish <($out/bin/colorctl completions fish) \
      --zsh <($out/bin/colorctl completions zsh)
  '';

  meta = with lib; {
    description = "Hardware control utility for Colorful AMD motherboards, providing RGB lighting and fan curve management";
    homepage = "https://github.com/Curstantine/colorctl";
    license = licenses.mit;
    mainProgram = "colorctl";
    platforms = platforms.linux;
  };
}
