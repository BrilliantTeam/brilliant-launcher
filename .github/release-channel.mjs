import fs from 'node:fs';

export function expectedChannel(version) {
    const pre = /-(alpha|beta)\.\d+$/.exec(version);
    if (pre) return pre[1];
    const [major, minor] = version.split('.').map(Number);
    return major > 1 || (major === 1 && minor >= 1) ? 'stable' : 'alpha';
}

export const PLATFORMS = ['windows', 'macos', 'linux', 'android'];

const DESKTOP = {
    windows: { os: 'windows-latest', args: '' },
    linux: { os: 'ubuntu-22.04', args: '--bundles appimage' },
    macos: { os: 'macos-latest', args: '--target universal-apple-darwin --bundles app,dmg' }
};

export function platformsOf(tag, version) {
    if (tag === `v${version}`) return PLATFORMS;
    if (!tag.startsWith(`v${version}-`)) return null;
    const list = tag.slice(version.length + 2).split('-');
    return list.every(p => PLATFORMS.includes(p)) && new Set(list).size === list.length ? list : null;
}

export function markedChannel(subject) {
    if (/\[(stable|正式)\]/i.test(subject)) return 'stable';
    if (/\[beta\]/i.test(subject)) return 'beta';
    if (/\[alpha\]/i.test(subject)) return 'alpha';
    return '';
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/').split('/').pop())) {
    const [version, subject, tag] = process.argv.slice(2);
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
        assert.deepStrictEqual(platformsOf('v1.3.0-beta.22', '1.3.0-beta.22'), PLATFORMS);
        assert.deepStrictEqual(platformsOf('v1.3.0-beta.23-android', '1.3.0-beta.23'), ['android']);
        assert.deepStrictEqual(platformsOf('v1.3.1-windows-macos', '1.3.1'), ['windows', 'macos']);
        assert.strictEqual(platformsOf('v1.3.0-beta.23', '1.3.0-beta.2'), null);
        assert.strictEqual(platformsOf('v1.3.0-beta.2', '1.3.0-beta.23'), null);
        assert.strictEqual(platformsOf('v1.3.1-ios', '1.3.1'), null);
        assert.strictEqual(platformsOf('v1.3.1-windows-windows', '1.3.1'), null);
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
    const platforms = platformsOf(tag || `v${version}`, version);
    if (!platforms) {
        console.log(`::error::標籤 ${tag} 要是 v${version}，或在後面加上要發布的平台（${PLATFORMS.join('、')}），例如 v${version}-windows-macos`);
        process.exit(1);
    }
    const desktop = Object.keys(DESKTOP).filter(p => platforms.includes(p)).map(platform => ({ platform, ...DESKTOP[platform] }));
    if (process.env.GITHUB_OUTPUT) {
        fs.appendFileSync(process.env.GITHUB_OUTPUT, [
            `version=${version}`,
            `tag=${tag || `v${version}`}`,
            `channel=${expected}`,
            `prerelease=${expected === 'stable' ? 'false' : 'true'}`,
            `desktop=${desktop.length > 0}`,
            `matrix=${JSON.stringify({ include: desktop })}`,
            `android=${platforms.includes('android')}`
        ].join('\n') + '\n');
    }
    console.log(`發布通道：${expected}，平台：${platforms.join('、')}`);
}
