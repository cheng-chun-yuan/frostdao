# FrostDAO - Hierarchical Threshold Signatures for Organizations
## Fork from -> (https://github.com/nickfarrow/yushan)
> **🏆 Winner at [BTC++ Taipei 2024](https://devpost.com/software/frostdao)**
> - **1st Place Overall**
> - **Best Use of Cryptography**

A production-grade FROST implementation with **Hierarchical Threshold Secret Sharing (HTSS)** - enabling organizational hierarchies for Bitcoin multisig.

**No single party ever knows the full secret key.**

## Overview

FrostDAO extends classical threshold signatures by introducing **ranks** (authority levels) to each share. This enables real-world organizational hierarchies where higher-ranked members are required for signing, while still maintaining the security guarantees of threshold cryptography.

### Key Features

- **Real Cryptography**: Built on `schnorr_fun` and `secp256kfun` - production-grade FROST implementation
- **Hierarchical Access Control**: Rank-based signing policies (CEO must approve, managers cannot act alone)
- **Birkhoff Interpolation**: Advanced mathematical foundation for hierarchical secret sharing
- **CLI & WASM Support**: Use from command line or integrate into web applications
- **No Trusted Dealer**: Distributed key generation - no single point of failure

## What's New: Hierarchical TSS (HTSS)

HTSS extends classical threshold signatures by introducing **ranks** (authority levels) to each share. This enables organizational hierarchies where:

- **Rank 0** = Highest authority (e.g., CEO, Board)
- **Rank 1** = High authority (e.g., C-Suite, Directors)
- **Rank 2** = Medium authority (e.g., Managers)
- **Rank 3+** = Lower authority (e.g., Team leads, Staff)

### TSS vs HTSS Comparison

| Feature | TSS (Traditional) | HTSS (Hierarchical) |
|---------|-------------------|---------------------|
| Shares | All equivalent | Each has a rank |
| Signing | Any t parties | Only valid rank combinations |
| Use case | Equal partnership | Organizational hierarchy |
| Math | Lagrange interpolation | Birkhoff interpolation |

## Quick Start

### Standard TSS (All parties equal)

```bash
# Build and install
cargo install --path .

# Keygen - 2-of-3 threshold (all parties run with their index)
yushan keygen-round1 --threshold 2 --n-parties 3 --my-index 1
yushan keygen-round1 --threshold 2 --n-parties 3 --my-index 2
yushan keygen-round1 --threshold 2 --n-parties 3 --my-index 3

# Exchange commitments, then shares...
yushan keygen-round2 --data '{"party_index":1,...} {"party_index":2,...} {"party_index":3,...}'
yushan keygen-finalize --data '<space-separated shares JSON>'

# Signing (any 2 parties)
yushan generate-nonce --session "tx1"
yushan sign --session "tx1" --message "Transfer $1M" --data '<nonces JSON>'
yushan combine --data '<shares JSON>'
```

### HTSS (Hierarchical) Mode

```bash
# Keygen - 3-of-4 with ranks [0, 1, 1, 2]
yushan keygen-round1 --threshold 3 --n-parties 4 --my-index 1 --rank 0 --hierarchical  # CEO
yushan keygen-round1 --threshold 3 --n-parties 4 --my-index 2 --rank 1 --hierarchical  # CFO
yushan keygen-round1 --threshold 3 --n-parties 4 --my-index 3 --rank 1 --hierarchical  # COO
yushan keygen-round1 --threshold 3 --n-parties 4 --my-index 4 --rank 2 --hierarchical  # Manager

# Same round2/finalize process...

# Valid signing combinations (ranks must satisfy: sorted_rank[i] <= i)
# OK:  CEO + CFO + COO     -> ranks [0,1,1] -> 0<=0, 1<=1, 1<=2 ✓
# OK:  CEO + CFO + Manager -> ranks [0,1,2] -> 0<=0, 1<=1, 2<=2 ✓
# FAIL: CFO + COO + Manager -> ranks [1,1,2] -> 1>0 at position 0 ✗
```

## HTSS Signing Rules

For threshold `t`, signers with sorted ranks `[r₀, r₁, ..., r_{t-1}]` are valid **if and only if**:

```
rᵢ ≤ i  for all positions i
```

This ensures higher-ranked members (lower numbers) are **required** for signing.

## Real-World Use Cases

### 1. Corporate Treasury Management

**Scenario**: A company manages a Bitcoin treasury worth $50M.

```
Threshold: 3-of-5
Ranks:
  - CEO (rank 0)
  - CFO (rank 1)
  - Treasurer (rank 1)
  - Finance Director (rank 2)
  - Accountant (rank 2)
```

**Valid combinations**:
- CEO + CFO + Treasurer (executive approval)
- CEO + CFO + Finance Director
- CEO + Treasurer + Accountant

**Invalid combinations**:
- CFO + Treasurer + Finance Director (no CEO = no rank-0)
- Any 3 without CEO involvement

**Benefit**: CEO must always be involved in treasury movements, but doesn't need all executives.

---

### 2. DAO Multi-Sig with Hierarchy

**Scenario**: A DAO with core team and community representatives.

```
Threshold: 4-of-7
Ranks:
  - Core Dev 1 (rank 0)
  - Core Dev 2 (rank 0)
  - Community Lead (rank 1)
  - Treasury Lead (rank 1)
  - Advisor 1 (rank 2)
  - Advisor 2 (rank 2)
  - Community Rep (rank 3)
```

**Policy**: At least one core dev must approve, plus community representation.

---

### 3. Family Trust / Estate Planning

**Scenario**: Family wealth management across generations.

```
Threshold: 2-of-4
Ranks:
  - Parent 1 (rank 0)
  - Parent 2 (rank 0)
  - Adult Child 1 (rank 1)
  - Adult Child 2 (rank 1)
```

**Valid combinations**:
- Either parent alone + any child
- Both parents (no children needed)

**Invalid combinations**:
- Both children without a parent

**Benefit**: Parents retain control; children can co-sign but not act alone.

---

### 4. Exchange Hot Wallet Security

**Scenario**: Crypto exchange managing hot wallet operations.

```
Threshold: 3-of-6
Ranks:
  - Security Officer (rank 0)
  - CTO (rank 0)
  - Senior DevOps 1 (rank 1)
  - Senior DevOps 2 (rank 1)
  - DevOps Engineer 1 (rank 2)
  - DevOps Engineer 2 (rank 2)
```

**Policy**: Every withdrawal requires Security Officer OR CTO approval.

---

### 5. Legal Document Signing

**Scenario**: Law firm signing contracts on behalf of clients.

```
Threshold: 2-of-4
Ranks:
  - Managing Partner (rank 0)
  - Senior Partner (rank 1)
  - Associate 1 (rank 2)
  - Associate 2 (rank 2)
```

**Policy**: Associates cannot sign without partner involvement.

---

### 6. Supply Chain Authorization

**Scenario**: Multi-party supply chain requiring sign-off from different stakeholders.

```
Threshold: 3-of-5
Ranks:
  - Manufacturer (rank 0)
  - Logistics Provider (rank 1)
  - Quality Inspector (rank 1)
  - Retailer (rank 2)
  - Insurance (rank 2)
```

**Policy**: Manufacturer must always authorize shipment releases.

---

## Technical Details

### Birkhoff Interpolation

HTSS uses **Birkhoff interpolation** instead of Lagrange interpolation. While Lagrange uses only point values, Birkhoff incorporates derivative information (ranks) to create hierarchical constraints.

When all ranks are 0, Birkhoff reduces to Lagrange, making HTSS backward-compatible with standard TSS.

### Files Structure

```
src/
├── birkhoff.rs   # Birkhoff interpolation & validation
├── keygen.rs     # DKG with HTSS support
├── signing.rs    # Signing with rank validation
├── main.rs       # CLI interface
├── wasm.rs       # WebAssembly bindings
└── storage.rs    # State persistence
```

## Workshop Outline

1. **Shamir's Secret Sharing** - Whiteboard introduction (~5 mins)
2. **Distributed Key Generation** - Sovereignty without a dealer (~5 min)
3. **Hands-on DKG** - Create a 2-of-3 on whiteboard (~15 min)
4. **HTSS Concepts** - Ranks and hierarchical signing (~10 min)
5. **Workshop** - Build and test using this repo!
6. **Q&A** - Discussion and closing

## Learning Goals

Participants will learn:

1. How polynomial secret sharing works (Shamir SSS)
2. How FROST distributes key generation without a trusted dealer
3. How HTSS adds organizational hierarchy to threshold signatures
4. The difference between Lagrange and Birkhoff interpolation
5. Real-world applications of hierarchical signing policies

## API Reference

### Keygen Round 1

```bash
yushan keygen-round1 \
  --threshold <T> \
  --n-parties <N> \
  --my-index <INDEX> \
  [--rank <RANK>] \        # Default: 0
  [--hierarchical]         # Enable HTSS mode
```

### Signing

```bash
yushan generate-nonce --session <SESSION_ID>
yushan sign --session <SESSION_ID> --message <MSG> --data '<nonces JSON>'
yushan combine --data '<signature shares JSON>'
```

## Security Notice

This is an **educational implementation** for learning threshold signatures and HTSS concepts.

**Do NOT use for production systems** without:
- Proper security audit
- Secure communication channels
- Hardware security modules (HSM) for key storage
- Comprehensive testing

## References

- [FROST: Flexible Round-Optimized Schnorr Threshold Signatures](https://eprint.iacr.org/2020/852)
- [Hierarchical Threshold Secret Sharing](https://www.cs.umd.edu/~gasMDa/htss.pdf)
- [Alice HTSS Implementation](https://github.com/getamis/alice/tree/master/crypto/tss/eddsa/frost)
- [schnorr_fun Library](https://github.com/LLFourn/secp256kfun)

## Acknowledgments

This project would not have been possible without the incredible work of:

- **[Frostsnap Team](https://frostsnap.com/)** - For building the excellent `schnorr_fun` and `secp256kfun` libraries that power the cryptographic foundation of this project. Their production-grade FROST implementation made hierarchical threshold signatures accessible.

- **[Nick Farrow](https://github.com/nickfarrow)** - For the original [Yushan](https://github.com/nickfarrow/yushan) workshop codebase that served as the foundation for this project. His educational approach to threshold signatures was invaluable.

Thank you for pushing Bitcoin cryptography forward! 🙏

## License

MIT
