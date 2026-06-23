// Room state management
import * as nostr from './nostr.js';
import * as encryption from './encryption.js';

// Room state
export const rooms = {
    dkg: createDkgRoom(),
    htss: createDkgRoom(),
    signing: createSigningRoom()
};

function createDkgRoom() {
    return {
        sub: null,
        room: null,
        connected: false,
        mySecretCoefficient: null,
        myIndex: null,
        data: {
            keygen_round1: new Map(),
            keygen_round2: new Map(),
            encryptionKeys: new Map(),
            decryptedShares: new Map(),
            keygen_round2_encrypted: new Map()
        }
    };
}

function createSigningRoom() {
    return {
        sub: null,
        room: null,
        connected: false,
        mySecret: null,
        myIndex: null,
        groupKeys: new Map(),
        nonces: new Map(),
        shares: new Map()
    };
}

// Subscribe to DKG/HTSS room
export function subscribeDkg(prefix, roomId, onUpdate) {
    const state = rooms[prefix];
    if (state.sub) state.sub.close();

    state.room = roomId;
    state.data.keygen_round1.clear();
    state.data.keygen_round2.clear();

    state.sub = nostr.subscribe(roomId,
        (data, event) => processDkgEvent(prefix, data, event, onUpdate),
        () => {
            state.connected = true;
            onUpdate('connected');
        }
    );
}

function processDkgEvent(prefix, data, nostrEvent, onUpdate) {
    const state = rooms[prefix];
    const eventType = data.type;
    const partyIndex = data.party_index;

    // Handle encrypted Round 2
    if (eventType === 'keygen_round2_encrypted') {
        processEncryptedRound2(prefix, data, onUpdate);
        return;
    }

    if (!state.data[eventType]) return;

    const existing = state.data[eventType].get(partyIndex);
    if (existing && existing.created_at <= nostrEvent.created_at) return;

    state.data[eventType].set(partyIndex, { ...data, created_at: nostrEvent.created_at });

    // Extract encryption pubkey from Round 1
    if (eventType === 'keygen_round1' && data.encryption_pubkey) {
        state.data.encryptionKeys.set(partyIndex, data.encryption_pubkey);
    }

    onUpdate('data');
}

function processEncryptedRound2(prefix, data, onUpdate) {
    const state = rooms[prefix];
    const fromIndex = data.party_index;
    const toIndex = data.to_index;

    if (state.myIndex && toIndex !== state.myIndex) return;

    if (state.mySecretCoefficient && state.data.encryptionKeys.has(fromIndex)) {
        try {
            const senderPubkey = state.data.encryptionKeys.get(fromIndex);
            const decrypted = encryption.decrypt(data.ciphertext, state.mySecretCoefficient, senderPubkey);
            state.data.decryptedShares.set(fromIndex, {
                from_index: fromIndex,
                share: decrypted,
                created_at: data.created_at
            });
        } catch (e) {
            console.error(`Failed to decrypt share from ${fromIndex}:`, e);
        }
    } else {
        state.data.keygen_round2_encrypted.set(fromIndex, data);
    }
    onUpdate('data');
}

// Set secret coefficient and decrypt pending shares
export function setSecretCoefficient(prefix, secretHex) {
    const state = rooms[prefix];
    state.mySecretCoefficient = secretHex;

    // Decrypt pending shares
    state.data.keygen_round2_encrypted.forEach((data, fromIndex) => {
        if (state.data.encryptionKeys.has(fromIndex)) {
            try {
                const senderPubkey = state.data.encryptionKeys.get(fromIndex);
                const decrypted = encryption.decrypt(data.ciphertext, state.mySecretCoefficient, senderPubkey);
                state.data.decryptedShares.set(fromIndex, {
                    from_index: fromIndex,
                    share: decrypted,
                    created_at: data.created_at
                });
            } catch (e) {}
        }
    });
}

// Post to room (with optional encryption for Round 2)
export async function postToRoom(prefix, content, eventType) {
    const state = rooms[prefix];
    if (!state.room) throw new Error('Not connected to room');

    // Encrypt Round 2 shares if secret is set
    if (eventType === 'keygen_round2' && state.mySecretCoefficient) {
        return postEncryptedRound2(prefix, content);
    }

    await nostr.publish(state.room, content);
}

async function postEncryptedRound2(prefix, data) {
    const state = rooms[prefix];
    const myIndex = data.party_index;
    let sent = 0;

    for (const shareData of data.shares) {
        const toIndex = shareData.to_index;
        const recipientPubkey = state.data.encryptionKeys.get(toIndex);
        if (!recipientPubkey) continue;

        try {
            const ciphertext = encryption.encrypt(shareData.share, state.mySecretCoefficient, recipientPubkey);
            await nostr.publish(state.room, {
                type: 'keygen_round2_encrypted',
                party_index: myIndex,
                to_index: toIndex,
                ciphertext
            });
            sent++;
        } catch (e) {
            console.error(`Failed to encrypt share for party ${toIndex}:`, e);
        }
    }
    return sent;
}

// Subscribe to signing room
export function subscribeSigning(roomId, myIndex, onUpdate) {
    const state = rooms.signing;
    if (state.sub) state.sub.close();

    state.room = roomId;
    state.myIndex = myIndex;
    state.nonces.clear();
    state.shares.clear();

    state.sub = nostr.subscribe(roomId,
        (data) => processSigningEvent(data, onUpdate),
        () => {
            state.connected = true;
            onUpdate('connected');
        }
    );
}

function processSigningEvent(data, onUpdate) {
    const state = rooms.signing;
    if (data.to_index && data.to_index !== state.myIndex) return;

    const fromIndex = data.party_index;

    if (data.type === 'signing_nonce_encrypted' && data.ciphertext) {
        decryptSigningData(fromIndex, data.ciphertext, state.nonces);
        onUpdate('nonce');
    } else if (data.type === 'signing_share_encrypted' && data.ciphertext) {
        decryptSigningData(fromIndex, data.ciphertext, state.shares);
        onUpdate('share');
    }
}

function decryptSigningData(fromIndex, ciphertext, targetMap) {
    const state = rooms.signing;
    if (!state.mySecret || !state.groupKeys.has(fromIndex)) return;

    try {
        const senderPubkey = state.groupKeys.get(fromIndex);
        const decrypted = encryption.decrypt(ciphertext, state.mySecret, senderPubkey);
        targetMap.set(fromIndex, JSON.parse(decrypted));
    } catch (e) {
        console.error(`Failed to decrypt from ${fromIndex}:`, e);
    }
}

// Setup signing encryption
export function setupSigningKeys(secretHex, keysText) {
    const state = rooms.signing;
    state.mySecret = secretHex;
    state.groupKeys.clear();

    for (const line of keysText.split('\n')) {
        const [idx, pubkey] = line.trim().split(':');
        if (idx && pubkey && encryption.isValidHex64(pubkey.trim())) {
            state.groupKeys.set(parseInt(idx), pubkey.trim());
        }
    }
    return state.groupKeys.size;
}

// Broadcast encrypted message to signing group
export async function broadcastToSigningGroup(type, data) {
    const state = rooms.signing;
    if (!state.room || !state.mySecret) throw new Error('Not configured');

    let sent = 0;
    for (const [toIndex, recipientPubkey] of state.groupKeys.entries()) {
        if (toIndex === state.myIndex) continue;

        try {
            const ciphertext = encryption.encrypt(data, state.mySecret, recipientPubkey);
            await nostr.publish(state.room, {
                type,
                party_index: state.myIndex,
                to_index: toIndex,
                ciphertext
            });
            sent++;
        } catch (e) {
            console.error(`Failed to send to party ${toIndex}:`, e);
        }
    }
    return sent;
}
