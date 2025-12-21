# FrostDAO CLI Reference

Complete reference for all CLI commands.

## Installation

```bash
cargo install --path .
frostdao --help
```

---

## Key Management

### btc-keygen

Generate a new Bitcoin Schnorr keypair (BIP340).

```bash
frostdao btc-keygen
```

**Output:**
- Public key (32-byte x-only, hex)
- Keypair saved to `.frost_state/bitcoin_keypair.json`

---

### btc-import-key

Import an existing secret key.

```bash
frostdao btc-import-key --secret <hex>
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--secret` | 32-byte secret key in hex (64 chars) |

---

### btc-pubkey

Display the stored public key.

```bash
frostdao btc-pubkey
```

---

## Address Commands

### btc-address

Get mainnet Taproot address (bc1p...).

```bash
frostdao btc-address
```

---

### btc-address-testnet

Get testnet Taproot address (tb1p...).

```bash
frostdao btc-address-testnet
```

---

### btc-address-signet

Get signet Taproot address (tb1p...).

```bash
frostdao btc-address-signet
```

---

### dkg-address

Get the DKG group Taproot address (testnet).

```bash
frostdao dkg-address --name <wallet_name>
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | Wallet/session name |

**Requires:** Completed DKG (`keygen-finalize`)

---

### dkg-balance

Check DKG group wallet balance on testnet.

```bash
frostdao dkg-balance --name <wallet_name>
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | Wallet/session name |

**Output:**
- DKG group address
- Total/Confirmed balance
- UTXO details
- Note about threshold signature requirements

**Requires:** Completed DKG (`keygen-finalize`)

---

## Transaction Commands

### btc-balance

Check testnet balance and UTXOs (single-signer wallet).

```bash
frostdao btc-balance
```

**Output:**
- Network
- Address
- Total/Confirmed balance
- UTXO details

---

### btc-send

Send Bitcoin on testnet.

```bash
frostdao btc-send --to <address> --amount <sats> [--fee-rate <sats/vbyte>]
```

**Parameters:**
| Parameter | Description | Required |
|-----------|-------------|----------|
| `--to` | Recipient Taproot address | Yes |
| `--amount` | Amount in satoshis | Yes |
| `--fee-rate` | Fee rate (sats/vbyte) | No (default: recommended) |

**Example:**
```bash
frostdao btc-send \
  --to tb1p3e44guscrytuum9q36tlx5kez9zvdheuwxlq9k9y4kud3hyckhtq63fz34 \
  --amount 10000 \
  --fee-rate 2
```

---

### btc-send-signet

Send Bitcoin on signet.

```bash
frostdao btc-send-signet --to <address> --amount <sats> [--fee-rate <sats/vbyte>]
```

---

## Signing Commands

### btc-sign

Sign a UTF-8 message with BIP340 Schnorr.

```bash
frostdao btc-sign --message "Hello, Bitcoin!"
```

---

### btc-sign-hex

Sign a hex-encoded message.

```bash
frostdao btc-sign-hex --message <hex_data>
```

---

### btc-sign-taproot

Sign a Taproot sighash (32 bytes).

```bash
frostdao btc-sign-taproot --sighash <hex>
```

---

### btc-verify

Verify a BIP340 signature.

```bash
frostdao btc-verify \
  --signature <64_byte_hex> \
  --public-key <32_byte_hex> \
  --message "Hello, Bitcoin!"
```

---

### btc-verify-hex

Verify with hex-encoded message.

```bash
frostdao btc-verify-hex \
  --signature <hex> \
  --public-key <hex> \
  --message <hex>
```

---

## DKG Commands

### keygen-round1

Generate polynomial and commitments for DKG.

```bash
frostdao keygen-round1 \
  --name <wallet_name> \
  --threshold <t> \
  --n-parties <n> \
  --my-index <i> \
  [--rank <r>] \
  [--hierarchical]
```

**Parameters:**
| Parameter | Description | Default |
|-----------|-------------|---------|
| `--name` | Wallet/session name (creates folder) | Required |
| `--threshold` | Minimum signers required | Required |
| `--n-parties` | Total number of parties | Required |
| `--my-index` | Your party index (1-based) | Required |
| `--rank` | HTSS rank (0=highest) | 0 |
| `--hierarchical` | Enable HTSS mode | false |

**Safety:** If a wallet with the same name exists, you'll be prompted to confirm replacement.

**Examples:**
```bash
# Standard TSS (2-of-3)
frostdao keygen-round1 --name treasury --threshold 2 --n-parties 3 --my-index 1

# HTSS (3-of-4 with ranks)
frostdao keygen-round1 --name corp_wallet --threshold 3 --n-parties 4 --my-index 1 --rank 0 --hierarchical
```

---

### keygen-round2

Exchange encrypted shares.

```bash
frostdao keygen-round2 --name <wallet_name> --data '<json>'
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | Wallet/session name (must match round1) |
| `--data` | JSON with all round1 commitments |

---

### keygen-finalize

Finalize DKG and derive keys.

```bash
frostdao keygen-finalize --name <wallet_name> --data '<json>'
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | Wallet/session name (must match round1) |
| `--data` | JSON with all round2 shares |

**Output:**
- Group public key
- Your secret share
- HTSS metadata (if hierarchical)
- `group_info.json` with parties ordered by rank

---

## Threshold Signing Commands

### generate-nonce

Generate a signing nonce for a session.

```bash
frostdao generate-nonce --session "tx-001"
```

**Important:** Never reuse session IDs!

---

### sign

Create a signature share.

```bash
frostdao sign \
  --session "tx-001" \
  --message "data to sign" \
  --data '<nonces_json>'
```

**Input Format:**
```json
{
  "nonces": [
    {"index": 1, "nonce": "..."},
    {"index": 2, "nonce": "..."}
  ],
  "public_key": "..."
}
```

---

### combine

Combine signature shares into final signature.

```bash
frostdao combine --data '<signature_shares_json>'
```

---

### verify

Verify a threshold signature.

```bash
frostdao verify \
  --signature <hex> \
  --public-key <hex> \
  --message "signed message"
```

---

## Resharing Commands

### reshare-round1

Generate sub-shares for new parties (run by old parties).

```bash
frostdao reshare-round1 \
  --source <wallet_name> \
  --new-threshold <t> \
  --new-n-parties <n> \
  --my-index <i>
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--source` | Source wallet name |
| `--new-threshold` | New threshold for reshared wallet |
| `--new-n-parties` | New total number of parties |
| `--my-index` | Your party index in the original wallet |

---

### reshare-finalize

Combine sub-shares to create new wallet (run by new parties).

```bash
frostdao reshare-finalize \
  --source <old_wallet> \
  --target <new_wallet> \
  --my-index <i> \
  --data '<round1_outputs_json>'
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--source` | Source wallet name (for metadata) |
| `--target` | New wallet name to create |
| `--my-index` | Your new party index |
| `--data` | JSON with round1 outputs from old parties |

---

## Share Recovery Commands

### recover-round1

Helper party generates sub-share for a lost party.

```bash
frostdao recover-round1 \
  --name <wallet_name> \
  --lost-index <i>
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | Wallet name |
| `--lost-index` | Index of the party who lost their share |

**Output:** JSON with `helper_index`, `lost_index`, `sub_share`, `rank`

---

### recover-finalize

Lost party combines sub-shares to recover their share.

```bash
frostdao recover-finalize \
  --source <wallet_name> \
  --target <new_wallet> \
  --my-index <i> \
  [--rank <r>] \
  [--hierarchical] \
  --data '<sub_shares_json>'
```

**Parameters:**
| Parameter | Description | Default |
|-----------|-------------|---------|
| `--source` | Source wallet name (for metadata) | Required |
| `--target` | New wallet name to create | Required |
| `--my-index` | Your party index (the one being recovered) | Required |
| `--rank` | Your HTSS rank | 0 |
| `--hierarchical` | Enable hierarchical mode | false |
| `--data` | JSON with sub-shares from helper parties | Required |

**Note:** For HTSS wallets with mixed ranks, uses Birkhoff interpolation.

---

## DKG Transaction Commands

### dkg-build-tx

Build an unsigned transaction for DKG threshold signing.

```bash
frostdao dkg-build-tx \
  --name <wallet_name> \
  --to <recipient_address> \
  --amount <satoshis> \
  [--fee-rate <sats_per_vbyte>]
```

**Parameters:**
| Parameter | Description | Default |
|-----------|-------------|---------|
| `--name` | DKG wallet name | Required |
| `--to` | Recipient Taproot address | Required |
| `--amount` | Amount in satoshis | Required |
| `--fee-rate` | Fee rate (sats/vbyte) | Auto |

**Output:** JSON with `session_id`, `sighash`, `unsigned_tx`

---

### dkg-nonce

Generate a signing nonce for DKG transaction.

```bash
frostdao dkg-nonce \
  --name <wallet_name> \
  --session <session_id>
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | DKG wallet name |
| `--session` | Session ID from dkg-build-tx |

**Output:** JSON with nonce data for this party

---

### dkg-sign

Create a signature share for DKG transaction.

```bash
frostdao dkg-sign \
  --name <wallet_name> \
  --session <session_id> \
  --sighash <hex> \
  --data '<nonces_json>'
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | DKG wallet name |
| `--session` | Session ID |
| `--sighash` | Transaction sighash (32-byte hex) |
| `--data` | JSON array of nonces from all signers |

**Output:** JSON with signature share

---

### dkg-broadcast

Combine signature shares and broadcast transaction.

```bash
frostdao dkg-broadcast \
  --name <wallet_name> \
  --unsigned-tx <hex> \
  --data '<signature_shares_json>'
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--name` | DKG wallet name |
| `--unsigned-tx` | Unsigned transaction hex from dkg-build-tx |
| `--data` | JSON array of signature shares |

**Output:** JSON with `txid` and broadcast status

---

## Storage Locations

DKG wallets are stored in named folders under `.frost_state/`:

```
.frost_state/
├── <wallet_name>/           # Each DKG wallet has its own folder
│   ├── group_info.json      # Public info (shareable)
│   ├── shared_key.bin       # Group public key
│   ├── paired_secret_share.bin  # Your secret share (keep private!)
│   ├── htss_metadata.json   # Ranks, threshold
│   ├── all_commitments.json # Round 1 data
│   └── round1_state.json    # DKG intermediate state
└── bitcoin_keypair.json     # Single-signer BIP340 keypair
```

### group_info.json

Generated after `keygen-finalize`, contains public info sorted by rank:

```json
{
  "name": "treasury",
  "group_public_key": "35a3e7ff...",
  "taproot_address_testnet": "tb1p...",
  "taproot_address_mainnet": "bc1p...",
  "threshold": 2,
  "total_parties": 3,
  "hierarchical": false,
  "parties": [
    {"index": 1, "rank": 0, "verification_share": "..."},
    {"index": 2, "rank": 0, "verification_share": "..."},
    {"index": 3, "rank": 0, "verification_share": "..."}
  ]
}
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (see message) |

---

## Environment

- Keys stored in `.frost_state/` (gitignored)
- Network API: mempool.space
- Testnet faucet: https://bitcoinfaucet.uo1.net/
