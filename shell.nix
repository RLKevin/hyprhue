{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  # Tools needed at build time (like compilers and pkg-config)
  nativeBuildInputs = with pkgs; [
    pkg-config
  ];

  # Libraries needed at runtime and build time
  buildInputs = with pkgs; [
    cargo
    rustc
    openssl
  ];
}
