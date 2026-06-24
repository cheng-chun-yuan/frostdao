# Miniscript Agent Payment Support

FrostDAO supports Miniscript as an optional policy and agent-payment initialization feature.

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

## Common Agent Payment Policies

The TUI includes common templates for local agent-payment drafts:

| Template | Policy |
|----------|--------|
| Agent hot path + DAO delayed recovery | `or(pk(AGENT),and(pk(DAO),older(144)))` |
| Agent requires DAO cosign | `and(pk(AGENT),pk(DAO))` |
| Agent, DAO, or recovery quorum | `thresh(2,pk(AGENT),pk(DAO),pk(RECOVERY))` |
| DAO immediate + agent after delay | `or(pk(DAO),and(pk(AGENT),older(144)))` |

`AGENT` is replaced with the agent's x-only pubkey entered in the TUI. `DAO` is replaced with the selected wallet's Frost-derived control pubkey. Other labels, such as `RECOVERY`, remain placeholders until a real recovery key registry is wired in.

## TUI Agent Payment Init

Run the TUI with the feature enabled:

```bash
cargo run --features miniscript-policy -- tui
```

From the home screen:

- select a wallet
- press `p` to open agent payment policy
- press `[` / `]` to cycle common policy templates
- enter agent label, agent pubkey, recipient, amount, daily limit, and agent index
- edit the policy field for a custom Miniscript policy
- press `Enter` to initialize the agent payment draft
- press `c` to copy the JSON draft
- press `Esc` to return home

The TUI uses the same Miniscript compiler as the CLI and derives the Frost key-path payment address from the selected wallet at the network-specific BIP-86 path: `m/86'/0'/0'/0/<agent_index>` on mainnet, or `m/86'/1'/0'/0/<agent_index>` on testnet, signet, and regtest. The Miniscript descriptor is included as a preview for the planned script path; script-path spending is still listed as future work below.

## Current Scope

Implemented:

- optional `miniscript-policy` feature
- Taproot policy parsing
- Taproot descriptor compilation
- descriptor sanity check
- CLI policy preview
- TUI agent payment policy draft
- common policy templates and custom policy entry
- all-feature tests in the quality gate

Not implemented yet:

- full key registry for persisted agent and recovery x-only pubkeys
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
