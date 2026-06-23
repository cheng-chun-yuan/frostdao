# Miniscript Policy Support

FrostDAO supports Miniscript as an optional policy-preview feature.

Miniscript is for Taproot script-path fallback policies. It does not replace FROST/HTSS key-path signing.

## Enable The Feature

```bash
cargo run --features miniscript-policy -- policy-compile --help
```

## Compile A Policy

```bash
cargo run --features miniscript-policy -- policy-compile \
  --policy 'thresh(2,pk(A),pk(B),pk(C))' \
  --internal-key INTERNAL
```

Output includes:

- normalized policy
- Taproot descriptor
- descriptor type
- SegWit version
- sanity-check result
- warning that spending is not wired yet

Example output:

```json
{
  "policy": "thresh(2,pk(A),pk(B),pk(C))",
  "descriptor": "tr(INTERNAL,multi_a(2,A,B,C))#mrdwws9c",
  "descriptor_type": "tr",
  "segwit_version": "V1",
  "is_taproot": true,
  "is_sane": true,
  "internal_key": "INTERNAL",
  "warning": "Compile/preview only. Script-path spending still requires witness/control-block integration."
}
```

## TUI Preview

Run the TUI with the feature enabled:

```bash
cargo run --features miniscript-policy -- tui
```

From the home screen:

- press `p` to open policy preview
- edit the policy
- press `Enter` to compile
- press `c` to copy the compiled JSON output
- press `Esc` to return home

The TUI preview uses the same compiler as the CLI.

## Current Scope

Implemented:

- optional `miniscript-policy` feature
- Taproot policy parsing
- Taproot descriptor compilation
- descriptor sanity check
- CLI policy preview
- TUI policy preview
- all-feature tests in the quality gate

Not implemented yet:

- translating key labels to concrete x-only pubkeys for address generation
- storing policy templates in wallet metadata
- Taproot script-path transaction building
- witness/control-block generation
- Miniscript satisfier integration
- script-path threshold signing in the TUI

## How This Fits FrostDAO

Use FROST/HTSS as the primary key-path spend:

```text
DAO policy satisfied -> FROST/HTSS key-path signature
```

Use Miniscript for advanced fallback paths:

```text
after timeout -> recovery key can spend
emergency board policy -> script-path spend
hashlock/timelock -> conditional settlement
```

Keep main wallet custody simple unless the user explicitly opts into advanced policy mode.
