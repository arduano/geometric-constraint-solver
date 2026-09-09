# Managed V3 compatibility fixture

`managed-compiler-envelope.json` preserves the exact pre-M97-authoring V3 compiler
receipt. The live fixture generator intentionally does not rewrite this directory.
The compiler compatibility tests authenticate these original bytes and prove the
first source-backed metadata edit upgrades to V4 without changing geometry inputs.
