{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    binaryen
    cargo
    clippy
    deno
    lld
    nodejs
    rust-analyzer
    rustc
    rustfmt
    wasm-bindgen-cli
  ];

  RUST_BACKTRACE = "1";
}
