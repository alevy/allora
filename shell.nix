# Shell expression for the Nix package manager
#
# This nix expression creates an environment with necessary packages installed:
#
#  * `tockloader`
#  * rust
#
# To use:
#
#  $ nix-shell
#

{ pkgs ? import <nixpkgs> {} }:

with builtins;
let
  inherit (pkgs) stdenv lib;
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rust_date = "2021-03-19";
  rust_channel = "nightly";
  rust_targets = [
    "aarch64-unknown-none"
  ];
  rust_build = (nixpkgs.rustChannelOfTargets rust_channel rust_date rust_targets).override {
    extensions = [ "rust-src" "llvm-tools-preview" ];
  };
in
  with pkgs;
  stdenv.mkDerivation {
    name = "aarchos-dev";

    buildInputs = [
      rust_build
      rustup
      qemu
    ];

  }
