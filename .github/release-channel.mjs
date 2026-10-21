import fs from 'node:fs';

export function expectedChannel(version) {
    const pre = /-(alpha|beta)\.\d+$/.exec(version);
    if (pre) return pre[1];
    const [major, minor] = version.split('.').map(Number);
    return major > 1 || (major === 1 && minor >= 1) ? 'stable' : 'alpha';
}

export function markedChannel(subject) {
    if (/\[(stable|正式)\]/i.test(subject)) return 'stable';
    if (/\[beta\]/i.test(subject)) return 'beta';
    if (/\[alpha\]/i.test(subject)) return 'alpha';
    return '';
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/').split('/').pop())) {
    const [version, subject] = process.argv.slice(2);
    if (version === '--test') {
        const assert = (await import('node:assert')).default;
        assert.strictEqual(expectedChannel('1.0.33'), 'alpha');
        assert.strictEqual(expectedChannel('1.1.0-alpha.2'), 'alpha');
        assert.strictEqual(expectedChannel('1.1.0-beta.1'), 'beta');
        assert.strictEqual(expectedChannel('1.1.0'), 'stable');
        assert.strictEqual(expectedChannel('2.0.0'), 'stable');
        assert.strictEqual(markedChannel('[alpha] 1.0.33：發布通道'), 'alpha');
        assert.strictEqual(markedChannel('1.1.0：[正式] 第一個正式版'), 'stable');
        assert.strictEqual(markedChannel('[Beta] 測試'), 'beta');
        assert.strictEqual(markedChannel('1.0.33：發布通道'), '');
        console.log('OK：發布通道判斷正常。');
        process.exit(0);
    }
    const expected = expectedChannel(version);
    const marked = markedChannel(subject || '');
    if (!marked) {
        console.log(`::error::commit 訊息第一行要標註 [alpha]、[beta] 或 [stable]，版本 ${version} 應標註 [${expected}]`);
        process.exit(1);
    }
    if (marked !== expected) {
        console.log(`::error::commit 訊息標註 [${marked}]，但版本號 ${version} 屬於 [${expected}]，請修正其中一個`);
        process.exit(1);
    }
    if (process.env.GITHUB_OUTPUT) {
        fs.appendFileSync(process.env.GITHUB_OUTPUT, `channel=${expected}\nprerelease=${expected === 'stable' ? 'false' : 'true'}\n`);
    }
    console.log(`發布通道：${expected}`);
}
