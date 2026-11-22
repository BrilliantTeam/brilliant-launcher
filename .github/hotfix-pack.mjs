import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

export function parseTag(tag, version) {
    const m = /^hotfix-(.+)-(\d+)$/.exec(tag);
    if (!m || m[1] !== version) return null;
    const build = Number(m[2]);
    return build >= 1 && build < 100000 ? build : null;
}

export function nativeChanges(files) {
    return files.filter((f) => f && !f.includes('/.kotlin/'));
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/').split('/').pop())) {
    const tag = process.argv[2];
    if (tag === '--test') {
        const assert = (await import('node:assert')).default;
        assert.strictEqual(parseTag('hotfix-1.3.0-beta.26-1', '1.3.0-beta.26'), 1);
        assert.strictEqual(parseTag('hotfix-1.3.1-12', '1.3.1'), 12);
        assert.strictEqual(parseTag('hotfix-1.3.0-beta.26-1', '1.3.0-beta.27'), null, '版本號要和程式一致');
        assert.strictEqual(parseTag('hotfix-1.3.0-beta.26', '1.3.0-beta.26'), null, '要有修正編號');
        assert.strictEqual(parseTag('hotfix-1.3.0-beta.26-0', '1.3.0-beta.26'), null);
        assert.deepStrictEqual(nativeChanges(['src-tauri/gen/android/.kotlin/sessions/x.salive', 'src-tauri/src/lib.rs', '']), ['src-tauri/src/lib.rs']);
        console.log('OK：介面修正標籤判斷正常。');
        process.exit(0);
    }
    const fail = (message) => {
        console.log(`::error::${message}`);
        process.exit(1);
    };
    const version = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
    const build = parseTag(tag || '', version);
    if (!build) fail(`標籤 ${tag} 要寫成 hotfix-${version}-修正編號，例如 hotfix-${version}-1`);
    const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
    const base = git('tag', '--list', `v${version}`, `v${version}-*`).split('\n').filter(Boolean)[0];
    if (!base) fail(`找不到 ${version} 的發佈標籤（v${version} 或 v${version}-平台），介面修正只能修補已發佈的版本`);
    const native = nativeChanges(git('diff', '--name-only', base, 'HEAD', '--', 'src-tauri', 'flatpak').split('\n'));
    if (native.length) fail(`自 ${base} 以來原生程式有變動，必須發佈正式版本，不能用介面修正：${native.join('、')}`);
    const files = git('diff', '--name-only', '--diff-filter=AM', base, 'HEAD', '--', 'src').split('\n').filter(Boolean).map((f) => f.slice('src/'.length));
    if (!files.length) fail(`自 ${base} 以來介面沒有任何變動`);
    fs.mkdirSync('release', { recursive: true });
    const name = `BrilliantLauncher_UI_${version}_${build}.zip`;
    execFileSync('zip', ['-X', '-q', path.resolve('release', name), ...files], { cwd: 'src' });
    if (process.env.GITHUB_OUTPUT) {
        fs.appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\nbuild=${build}\nbase=${base}\n`);
    }
    console.log(`以 ${base} 為基準，打包 ${files.length} 個介面檔案到 ${name}：\n${files.join('\n')}`);
}
