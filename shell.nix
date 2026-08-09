{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    binaryen
    cargo
    clippy
    lld
    nodejs
    rust-analyzer
    rustc
    rustfmt
    trunk
    wasm-bindgen-cli
  ];

  RUST_BACKTRACE = "1";
}
