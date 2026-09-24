{
  description = "GeoSolve deep-mutation fuzzing campaign: four libFuzzer targets, parallel, core-pinned via taskset. Run with `nix run github:arduano/geometric-constraint-solver#fuzz-campaign`.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: let
    # This experimental Nix build (2.34.8) exposes no `builtins.currentSystem`
    # and `NIX_SYSTEM` is empty during evaluation, so default to the fuzzing
    # host's architecture (Intel Xeon E5-2697 v4, x86_64-linux). If `NIX_SYSTEM`
    # is set during an actual `nix run`/`nix build`, it is honored instead. An
    # explicit `system` lets nixpkgs import without needing `currentSystem`.
    system =
      let
        envSys = builtins.getEnv "NIX_SYSTEM";
      in
      if (envSys != "" && envSys != null) then envSys else "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
    };

    # Toolchain from nixpkgs's `pkgs.rust` module. This experimental nixpkgs has
    # no `rustChannelOf`/`rustToolchain`, and `pkgs.rust` is a configuration set
    # (not a derivation); the actual components live under
    # `pkgs.rust.packages.stable` (rustc, cargo, rustfmt, clippy). cargo-fuzz
    # builds standard libFuzzer harnesses on stable; rustfmt/clippy back the
    # optional --check phase. The toolchain ships standalone store paths, so no
    # RUSTUP_TOOLCHAIN is required.
    rustc = pkgs.rust.packages.stable.rustc;
    cargo = pkgs.rust.packages.stable.cargo;
    rustfmt = pkgs.rust.packages.stable.rustfmt;
    clippy = pkgs.rust.packages.stable.clippy;

    # Bundle the campaign script + the Rust source tree the fuzz crate needs
    # (crates/, fuzz/ source, manifests, rust-toolchain.toml) into one package,
    # plus a wrapper that installs the toolchain/clang/taskset environment and
    # execs the script. The script then copies the crate per-target under the
    # current working directory and runs everything in parallel there.
    campaign = pkgs.runCommand "fuzz-campaign-runner" { } ''
      mkdir -p "$out/bin" "$out/fuzz"
      cp ${self}/scripts/fuzz-campaign.sh "$out/bin/"
      cp -r ${self}/crates "$out/"
      cp -r ${self}/fuzz/Cargo.toml ${self}/fuzz/Cargo.lock ${self}/fuzz/fuzz.toml ${self}/fuzz/.gitignore "$out/fuzz/"
      cp -r ${self}/fuzz/src ${self}/fuzz/fuzz_targets ${self}/fuzz/tests "$out/fuzz/"
      # Golden seeds (gitignored locally, so best-effort: the script prefills
      # them from FuzzInput::golden_seeds() when this copy is absent, e.g. on a
      # fresh clone fetched from GitHub).
      cp -r ${self}/fuzz/corpus "$out/fuzz/" 2>/dev/null || true
      cp ${self}/Cargo.toml ${self}/Cargo.lock "$out/" 2>/dev/null || true
      cp ${self}/rust-toolchain.toml "$out/" 2>/dev/null || true

      cat > "$out/bin/fuzz-campaign" <<'EOF'
      #!${pkgs.bash}/bin/bash
      set -euo pipefail
      # Include the Nix store bash first so `bash` resolves deterministically
      # (e.g. the campaign script's unqualified `setsid bash`), and so the
      # interpreter is always a real executable -- `/bin/bash` does not exist
      # inside the Nix store, which is what this wrapper is built for.
      export PATH="${pkgs.bash}/bin:${pkgs.llvmPackages.clang}/bin:${rustc}/bin:${cargo}/bin:${rustfmt}/bin:${clippy}/bin:${pkgs.cargo-fuzz}/bin:${pkgs.util-linux}/bin:${pkgs.coreutils}/bin:${pkgs.procps}/bin:$PATH"
      export CC="${pkgs.llvmPackages.clang}/bin/clang"
      export CXX="${pkgs.llvmPackages.clang}/bin/clang++"
      export LLVM_CONFIG="${pkgs.llvmPackages.llvm}/bin/llvm-config"
      export RUST_BACKTRACE=1
      # Invoke the campaign script with the store bash explicitly rather than
      # `exec "$0.sh"`: that would re-read fuzz-campaign.sh's shebang
      # (#!/usr/bin/env bash) and exec `/usr/bin/env`, which is not guaranteed
      # to exist in the store. Running it as a script argument skips the
      # shebang entirely.
      exec "${pkgs.bash}/bin/bash" "$0.sh" "$@"
      EOF
      chmod +x "$out/bin/fuzz-campaign"
    '';

    # Bundle a campaign's receipts (crash artifacts, corpus, run logs and the
    # fuzz crate source) into a zip for offline analysis. Unlike the campaign
    # runner, this package ships only the script: the script reads the whole
    # campaign output from OUT_ROOT in the current directory, so the store
    # package needs nothing else. The wrapper installs just coreutils, findutils
    # and zip.
    bundle = pkgs.runCommand "fuzz-bundle-runner" { } ''
      mkdir -p "$out/bin"
      cp ${self}/scripts/fuzz-bundle.sh "$out/bin/"

      cat > "$out/bin/fuzz-bundle" <<'EOF'
      #!${pkgs.bash}/bin/bash
      set -euo pipefail
      export PATH="${pkgs.bash}/bin:${pkgs.coreutils}/bin:${pkgs.findutils}/bin:${pkgs.zip}/bin:$PATH"
      exec "${pkgs.bash}/bin/bash" "$0.sh" "$@"
      EOF
      chmod +x "$out/bin/fuzz-bundle"
    '';

    bundleApp = {
      type = "app";
      program = "${bundle}/bin/fuzz-bundle";
    };

    campaignApp = {
      type = "app";
      program = "${campaign}/bin/fuzz-campaign";
    };

    fuzzDevShell = pkgs.mkShell {
      name = "fuzz";
      packages = [
        rustc
        cargo
        rustfmt
        clippy
        pkgs.cargo-fuzz
        pkgs.llvmPackages.clang
        pkgs.llvmPackages.llvm
        pkgs.util-linux
        pkgs.procps
        pkgs.coreutils
      ];
      RUST_BACKTRACE = "1";
      CC = "${pkgs.llvmPackages.clang}/bin/clang";
      CXX = "${pkgs.llvmPackages.clang}/bin/clang++";
      LLVM_CONFIG = "${pkgs.llvmPackages.llvm}/bin/llvm-config";
    };
  in {
    # One-command campaign:
    #   nix run github:arduano/geometric-constraint-solver#fuzz-campaign
    #   nix run github:arduano/geometric-constraint-solver#fuzz-campaign -- \
    #       FUZZ_DURATION=12h FUZZ_OUT_ROOT=campaign
    apps.${system} = {
      default = campaignApp;
      "fuzz-campaign" = campaignApp;
      "fuzz-bundle" = bundleApp;
    };

    packages.${system} = {
      default = campaign;
      "fuzz-campaign" = campaign;
      "fuzz-bundle" = bundle;
    };

    devShells.${system} = {
      default = fuzzDevShell;
      "fuzz" = fuzzDevShell;
    };
  };
}
