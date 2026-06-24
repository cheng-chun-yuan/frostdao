# TUI Keymap

This is the maintained keyboard convention for FrostDAO TUI.

## Global Convention

| Key | Meaning |
|-----|---------|
| `Enter` | Primary action, continue, confirm, compile, generate, or complete |
| `Esc` | Back, cancel, close popup, or leave current flow |
| `↑/↓` | Move selection |
| `j` / `k` | Move down/up, Vim-style alternative to arrows |
| `Tab` / `Shift-Tab` | Next/previous form field |
| `Space` | Toggle checkbox or binary option |
| `c` | Copy current selected/output value when the screen has copyable output |
| `r` | Refresh, retry, or reject depending on screen context |
| `q` | Quit only from home; close QR popup when shown |

Avoid function keys for required actions. Use lowercase shortcuts in help text. Uppercase aliases may work in some older flows, but new UI should document lowercase.

## Home

| Key | Action |
|-----|--------|
| `↑/↓`, `j/k` | Select wallet |
| `Enter` | Open selected wallet |
| `g` | Create local wallet |
| `n` | Select network |
| `o` | Open Nostr room |
| `p` | Open Miniscript agent payment policy, only with `miniscript-policy` feature |
| `h` | Reshare selected wallet |
| `s` | Send from selected wallet |
| `a` | View derived addresses |
| `m` | Show mnemonic backup flow |
| `c` | Copy selected wallet address |
| `r` | Refresh selected wallet balance |
| `R` | Reload wallet list |
| `q` | Quit |

`N` still opens the Nostr room as a legacy alias, but visible help uses `o`.

## Wallet Details

| Key | Action |
|-----|--------|
| `↑/↓`, `j/k` | Select action |
| `Enter` | Run selected action |
| `b` | Fetch balance |
| `c` | Copy wallet address |
| `v` | Show QR code |
| `Esc` | Back |

Delete confirmation:

| Key | Action |
|-----|--------|
| Type wallet name | Arm deletion only when it exactly matches the selected wallet |
| `Enter` | Delete only after the typed name matches |
| `Backspace` | Edit typed confirmation |
| `Esc` | Cancel delete confirmation |

## Chain Select

| Key | Action |
|-----|--------|
| `↑/↓`, `j/k` | Select network |
| `Enter` | Confirm network |
| `Esc` | Cancel |

The selector shows the selected network policy before confirmation: testnet4/testnet3/signet use remote mempool.space UTXOs, regtest uses the local Esplora/mempool API from `FROSTDAO_REGTEST_MEMPOOL_API`, and mainnet requires explicit real-funds opt-in.

## Forms And Wizards

Applies to keygen, reshare, send, and Nostr room configuration.

| Key | Action |
|-----|--------|
| `Tab` | Next field |
| `Shift-Tab` | Previous field |
| `Enter` | Submit current step |
| `Esc` | Back/cancel |
| `Space` | Toggle focused binary option |
| `Ctrl+u` | Clear focused text input or paste area |

Multiline paste areas accept typed text and terminal paste. Use `Ctrl+u` to clear a bad paste before retrying. Screen help decides whether `Enter` submits the step or inserts a newline.

## Text Editing

Applies when a single-line input or multiline paste area is focused.

| Key | Action |
|-----|--------|
| `Backspace` | Delete before cursor |
| `Delete` | Delete under cursor |
| `←/→` | Move cursor left/right |
| `↑/↓` | Move cursor line in multiline paste areas |
| `Home` / `End` | Move to start/end of current field or line |
| `Ctrl+u` | Clear focused field or paste area |

## Keygen

| Phase | Key | Action |
|-------|-----|--------|
| Mode select | `↑/↓` | Toggle TSS/HTSS |
| Mode select | `1` | Select TSS |
| Mode select | `2` | Select HTSS |
| Mode select | `Enter` | Continue |
| Parameters | `Tab`, `Shift-Tab` | Move fields |
| Parameters | `Enter` | Generate local wallet or continue distributed flow |
| Round output | `c` | Copy JSON output |
| Round output | `Enter` | Continue |
| Any phase | `Esc` | Back/cancel |

## Reshare

| Phase | Key | Action |
|-------|-----|--------|
| Mode select | `↑/↓`, `j/k` | Select local or distributed reshare |
| Mode select | `Enter` | Continue |
| Local setup | `Tab`, `Shift-Tab` | Move fields |
| Local setup | `↑/↓` | Select source wallet when wallet field is focused |
| Distributed setup | `Tab`, `Shift-Tab` | Move fields |
| Distributed setup | `j/k` | Select source wallet when wallet field is focused |
| Round output | `c` | Copy JSON output |
| Round output | `Enter` | Go to finalize as new party |
| Finalize | `Space` | Toggle HTSS when hierarchy field is focused |
| Finalize | `Enter` | Finalize new share |
| Any phase | `Esc` | Back/cancel |

## Send

| Phase | Key | Action |
|-------|-----|--------|
| Select wallet | `↑/↓`, `j/k` | Select source wallet |
| Select signers | `↑/↓`, `j/k` | Select party |
| Select signers | `Space` | Toggle selected party |
| Select address | `↑/↓`, `j/k` | Select root or tweaked HD address |
| Script config | `↑/↓`, `j/k` | Select script type |
| Script config | `Space` | Apply selected script type |
| Script config | `Tab` | Move script option field |
| Script config | `m` | Toggle relative timelock mode |
| Transaction details | `Tab`, `Shift-Tab` | Move address/amount fields |
| Transaction review | `y` | Confirm only after every signer sees the same review, then locally sign and attempt broadcast |
| Sighash | `c` | Copy sighash |
| Nonce output | `c` | Copy nonce JSON |
| Signature share | `c` | Copy signature share JSON |
| Complete | `c` | Copy TXID, or raw transaction when broadcast failed |
| Any phase | `Enter` | Continue/submit |
| Any phase | `Esc` | Back/cancel |

## Address List

| Key | Action |
|-----|--------|
| `↑/↓`, `j/k` | Select derived address |
| `c` | Copy selected address |
| `b` | Fetch selected address balance |
| `+`, `a` | Add next derived address |
| `-`, `x` | Remove last derived address |
| `Esc` | Back |

## Mnemonic Backup

| Key | Action |
|-----|--------|
| `↑/↓`, `j/k` | Select party before reveal |
| `Enter` | Continue through party select, warning, reveal, and done |
| `Esc` | Cancel/back |

The TUI does not provide a mnemonic copy shortcut. This keeps backup words deliberate and avoids accidental clipboard persistence.

## Copyable Outputs

| Screen | Key |
|--------|-----|
| Keygen round output | `c` copies JSON |
| Reshare round output | `c` copies JSON |
| Send sighash | `c` copies sighash |
| Send nonce output | `c` copies JSON |
| Send signature share | `c` copies JSON |
| Send complete | `c` copies TXID |
| Address list | `c` copies selected address |
| Miniscript agent payment draft | `c` copies initialized JSON draft |

## Miniscript Agent Payment Policy

Requires:

```bash
cargo run --features miniscript-policy -- tui
```

| Key | Action |
|-----|--------|
| `p` from Home | Open agent payment policy screen |
| `Tab`, `Shift-Tab` | Move between agent label, pubkey, payment fields, and custom policy input |
| `[` / `]` | Cycle common policy templates |
| `Enter` | Initialize agent payment draft |
| `c` | Copy initialized JSON draft |
| `Esc` | Back to Home |

The policy input remains editable. Use the templates as starting points, then type a custom Miniscript policy when the policy field is focused.

## Nostr Room

| Phase | Key | Action |
|-------|-----|--------|
| Configure | `Tab` | Next field |
| Configure | `Enter` | Join room |
| Configure | `Esc` | Back |
| Waiting | `Space` | Add a local test participant in local simulation mode |
| Waiting | `Esc` | Leave room |
| Ready | `k` | Start distributed keygen |
| Ready | `s` | Start distributed signing |
| Ready | `Esc` | Leave room |

The room info line shows room ID, party index, threshold, scheme, rank, and
transport. Current TUI room joins are guarded as `Scheme: TSS` with `Rank: n/a`;
room join metadata is public, while signing nonce/share payloads are encrypted.
The configure status line shows `Blocked` until the room ID, party index,
threshold, and party count form a valid room.

## Nostr Keygen

| Key | Action |
|-----|--------|
| `Enter` | Start or continue current DKG phase |
| `r` | Retry from setup |
| `Esc` | Return to Nostr room |

## Nostr Signing

| Key | Action |
|-----|--------|
| `↑/↓`, `j/k` | Select wallet where applicable |
| `Tab`, `Shift-Tab` | Move recipient/amount field while configuring a proposal |
| `Enter` | Continue/done |
| `p` | Propose transaction, in role select |
| `c` | Consent role in role select, or copy TXID on completion |
| `Ctrl+u` | Clear focused recipient or amount field while configuring a proposal |
| `y` | Consent after reviewing proposal fingerprint |
| `r` | Publish a proposal rejection or retry where shown |
| `Esc` | Back/cancel |

The configure and review screens show the source path and source address. Send review also shows the child x-only pubkey fingerprint for HD sources. If the proposer selected an HD-derived source, the Nostr proposal uses that same derived path; consent only after every signer sees the same source path, source address, destination, amount, and fingerprint.
Regtest Nostr proposals require `FROSTDAO_REGTEST_MEMPOOL_API`; without it, the configure screen blocks publishing and shows the missing setting.
