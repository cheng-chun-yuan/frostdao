// Nostr connection and publishing utilities
import { SimplePool, generateSecretKey, getPublicKey, finalizeEvent } from 'https://esm.sh/nostr-tools@2.7.2';

// Single relay is fine - all data is E2E encrypted with NIP-44
export const RELAYS = ['wss://relay.damus.io'];

export const pool = new SimplePool();

// Generate ephemeral identity for this session
const sk = generateSecretKey();
const pk = getPublicKey(sk);

export function getSecretKey() { return sk; }
export function getPublicKey() { return pk; }

// Publish event to relays
export async function publish(roomId, content) {
    const event = finalizeEvent({
        kind: 1,
        created_at: Math.floor(Date.now() / 1000),
        tags: [['r', roomId]],
        content: typeof content === 'string' ? content : JSON.stringify(content)
    }, sk);

    await Promise.any(pool.publish(RELAYS, event));
    return event;
}

// Subscribe to a room
export function subscribe(roomId, onEvent, onEose) {
    return pool.subscribeMany(RELAYS, [
        { kinds: [1], '#r': [roomId], since: Math.floor(Date.now() / 1000) - 3600 }
    ], {
        onevent(event) {
            try {
                const data = JSON.parse(event.content);
                onEvent(data, event);
            } catch (e) {}
        },
        oneose: onEose
    });
}
