import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const PUBLIC_KEY = 'lunfr7RSo4DILBeY6dlwBW8KpSFkbSMweYK0DLB/uMU=';
const [dir, output = 'SIGNATURES.json'] = process.argv.slice(2);
const pem = String(process.env.UPDATE_SIGNING_KEY || '').replace(/\\n/g, '\n').trim();
if (!pem.includes('PRIVATE KEY')) {
    console.error('::error::缺少 UPDATE_SIGNING_KEY，拒絕發佈沒有數位簽章的版本');
    process.exit(1);
}
const privateKey = crypto.createPrivateKey(pem);
const derived = crypto.createPublicKey(privateKey).export({ type: 'spki', format: 'der' });
if (derived.subarray(derived.length - 32).toString('base64') !== PUBLIC_KEY) {
    console.error('::error::UPDATE_SIGNING_KEY 與程式內建的公鑰不符');
    process.exit(1);
}
const signatures = {};
for (const name of fs.readdirSync(dir).filter((n) => !/^SIGNATURES.*\.json$/.test(n)).sort()) {
    const sha = crypto.createHash('sha256').update(fs.readFileSync(path.join(dir, name))).digest('hex');
    signatures[name] = crypto.sign(null, Buffer.from(`brilliant-update-v1\n${name}\n${sha}`), privateKey).toString('base64');
}
fs.writeFileSync(path.join(dir, output), JSON.stringify(signatures, null, 1) + '\n');
console.log(`已簽署 ${Object.keys(signatures).length} 個檔案`);
