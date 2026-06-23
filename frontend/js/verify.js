// Schnorr signature verification
import * as secp from 'https://esm.sh/@noble/secp256k1@2.1.0';

window.verifySignature = async () => {
    const pubKeyHex = document.getElementById('verify-pubkey').value.trim();
    const message = document.getElementById('verify-message').value.trim();
    const sigHex = document.getElementById('verify-signature').value.trim();
    const resultDiv = document.getElementById('verify-result');

    if (!pubKeyHex || !message || !sigHex) {
        alert('Fill all fields');
        return;
    }

    try {
        if (!/^[0-9a-fA-F]{64}$/.test(pubKeyHex)) throw new Error('Public key: 64 hex chars');
        if (!/^[0-9a-fA-F]{128}$/.test(sigHex)) throw new Error('Signature: 128 hex chars');

        const pubKey = secp.etc.hexToBytes(pubKeyHex);
        const sig = secp.etc.hexToBytes(sigHex);
        const msgBytes = new TextEncoder().encode(message);
        const valid = secp.schnorr.verify(sig, msgBytes, pubKey);

        resultDiv.innerHTML = valid
            ? `<div class="result-box"><h3>&#10003; VALID</h3><p style="color:#aaa">Signature verified!</p></div>`
            : `<div class="result-box error"><h3>&#10007; INVALID</h3><p style="color:#aaa">Verification failed</p></div>`;
        resultDiv.style.display = 'block';
    } catch (e) {
        resultDiv.innerHTML = `<div class="result-box error"><h3>Error</h3><p>${e.message}</p></div>`;
        resultDiv.style.display = 'block';
    }
};

window.clearVerifyForm = () => {
    ['verify-pubkey', 'verify-message', 'verify-signature'].forEach(id => {
        document.getElementById(id).value = '';
    });
    document.getElementById('verify-result').style.display = 'none';
};
