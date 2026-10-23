const RULES = [
    {
        test: /Could not reserve enough space for .*object heap|Invalid maximum heap size|Initial heap size set to a larger value than the maximum/,
        title: '記憶體上限設得太高',
        detail: () => '電腦無法提供這麼多記憶體給遊戲。請到「設定」把記憶體上限調低（一般 4096～6144 MB 就夠用）。'
    },
    {
        test: /java\.lang\.OutOfMemoryError/,
        title: '記憶體不足',
        detail: () => '遊戲用完了分配的記憶體。請到「設定」把記憶體上限調高（建議 4096～8192 MB），或減少光影、材質包與高耗能模組。'
    },
    {
        test: /UnsupportedClassVersionError|has been compiled by a more recent version of the Java Runtime/,
        title: 'Java 版本太舊',
        detail: () => '這個 Minecraft 版本需要更新的 Java。請到「設定 → Java 執行檔」選「自動」，讓啟動器下載正確的版本。'
    },
    {
        test: /Incompatible mods? found|incompatible mod set|requires (?:version|any version)/i,
        title: '模組缺少前置或版本不相容',
        detail: (lines) => {
            const reasons = lines
                .filter((line) => /^\s*-\s*(?:Mod|Install|Replace|Remove)\b/.test(line) || /requires (?:version|any version)/i.test(line))
                .map((line) => line.trim().replace(/^-\s*/, ''))
                .slice(0, 6);
            return `Fabric 拒絕載入目前的模組組合。${reasons.length ? `\n${reasons.map((r) => `・${r}`).join('\n')}` : ''}\n請到實例頁的「模組與資源」安裝缺少的前置模組，或移除不支援這個版本的模組。`;
        }
    },
    {
        test: /Mixin apply for mod \S+ failed|MixinApplyError|InvalidMixinException|Mixin transformation of .+ failed/,
        title: '模組互相衝突',
        detail: (lines) => {
            const mods = [...new Set(lines.flatMap((line) => [...line.matchAll(/Mixin apply for mod (\S+) failed/g)].map((m) => m[1])))];
            return `${mods.length ? `出問題的模組：${mods.join('、')}。` : ''}有模組修改遊戲的方式互相衝突，或模組不支援目前的 Minecraft 版本。請先停用最近新增的模組再試一次。`;
        }
    },
    {
        test: /Duplicate mod|Found duplicate mods|duplicate mod ids?/i,
        title: '有重複的模組',
        detail: () => '同一個模組被放了兩份（常見於不同版本的同名 jar）。請到實例頁的「模組與資源」刪掉舊的那一份。'
    },
    {
        test: /NoClassDefFoundError|ClassNotFoundException|NoSuchMethodError|NoSuchFieldError/,
        title: '模組版本與遊戲不符',
        detail: (lines) => {
            const hit = lines.find((line) => /NoClassDefFoundError|ClassNotFoundException|NoSuchMethodError|NoSuchFieldError/.test(line));
            return `有模組是為其他 Minecraft 版本製作的，找不到它需要的程式碼。${hit ? `\n${hit.trim().slice(0, 160)}` : ''}\n請確認每個模組都支援這個實例的版本。`;
        }
    },
    {
        test: /GLFW error 65542|WGL: The driver does not appear to support OpenGL|Pixel format not accelerated|Failed to create window|OpenGL [0-9.]+ is not supported/,
        title: '顯示卡驅動程式問題',
        detail: () => '遊戲無法建立 OpenGL 視窗。請更新顯示卡驅動程式；筆電請確認遊戲使用獨立顯示卡。'
    }
];

export function diagnose(lines) {
    const text = lines.join('\n');
    const findings = RULES.filter((rule) => rule.test.test(text)).map((rule) => ({ title: rule.title, detail: rule.detail(lines) }));
    const exception = [...lines].reverse().find((line) => /\b[\w.]+(?:Exception|Error)\b/.test(line) && !/^\s*at /.test(line));
    return { findings, exception: exception ? exception.trim().slice(0, 200) : '' };
}

const NO_INSTALL = new Set(['minecraft', 'java', 'fabricloader', 'fabric']);
const GPU_DLL = /ig\w*icd(?:32|64)|atio6axx|atioglxx|nvoglv(?:32|64)|amdxc64|libnvidia|iris_dri|radeonsi/i;
const newer = (a, b) => b.version.localeCompare(a.version, undefined, { numeric: true });

export function plan({ lines = [], report = '', native = '', mods = [], memoryMb = 0, totalMb = 0, recommendedMb = 4096, preferDgpu = false }) {
    const all = [...report.split('\n'), ...lines];
    const text = all.join('\n');
    const { findings, exception } = diagnose(all);
    const enabled = mods.filter((m) => m.enabled && m.id);
    const byId = (id) => enabled.find((m) => m.id === id);
    const fixes = [];
    const add = (fix) => { if (!fixes.some((f) => f.kind === fix.kind && f.target === fix.target)) fixes.push(fix); };
    const disable = (mod, why) => mod && !mod.managed && add({ kind: 'disable', target: mod.file, label: `停用「${mod.name}」${why}` });
    const update = (mod) => mod && !mod.managed && add({ kind: 'update', target: mod.file, label: `更新「${mod.name}」到相容版本（找不到就停用）` });
    const loader = () => add({ kind: 'loader', target: '', label: '把 Fabric Loader 更新到這個 Minecraft 版本的最新穩定版' });

    if (/Could not reserve enough space|Invalid maximum heap size|Initial heap size set to a larger value/.test(text) && memoryMb > recommendedMb) {
        add({ kind: 'memory', target: recommendedMb, label: `把記憶體上限調成建議的 ${recommendedMb} MB` });
    } else if (/java\.lang\.OutOfMemoryError/.test(text)) {
        const cap = totalMb ? Math.floor((totalMb * 0.6) / 512) * 512 : 8192;
        const mb = Math.min(Math.max(memoryMb, 2048) + 2048, cap);
        if (mb > memoryMb) add({ kind: 'memory', target: mb, label: `把記憶體上限調高到 ${mb} MB` });
    }
    const classVersion = /class file version (\d+)/.exec(text);
    if (classVersion && /UnsupportedClassVersionError|more recent version of the Java Runtime/.test(text)) {
        const major = Number(classVersion[1]) - 44;
        add({ kind: 'java', target: major, label: `改用 Java ${major}（啟動器會自動下載）` });
    }
    for (const line of all) {
        const req = /Mod '[^']*' \(([\w-]+)\)[^\n]*? requires (?:.+?) of (?:mod '[^']*' \()?([\w-]+)\)?,? (which is missing|but only the wrong version)/.exec(line);
        if (req) {
            const [, owner, dep, how] = req;
            if (dep === 'fabricloader') loader();
            else if (NO_INSTALL.has(dep)) update(byId(owner));
            else if (how.startsWith('which') && !byId(dep)) add({ kind: 'install', target: dep, label: `安裝缺少的前置模組 ${dep}` });
            else update(byId(dep));
        }
        const install = /^\s*-\s*Install ([\w-]+),/.exec(line);
        if (install?.[1] === 'fabricloader') loader();
        if (install && !NO_INSTALL.has(install[1]) && !byId(install[1])) add({ kind: 'install', target: install[1], label: `安裝缺少的前置模組 ${install[1]}` });
        const replace = /^\s*-\s*Replace mod '[^']*' \(([\w-]+)\)/.exec(line);
        if (replace) update(byId(replace[1]));
        const remove = /^\s*-\s*Remove mod '[^']*' \(([\w-]+)\)/.exec(line);
        if (remove) disable(byId(remove[1]), '（與其他模組不相容）');
        for (const m of line.matchAll(/Mixin apply for mod ([\w-]+) failed/g)) disable(byId(m[1]), '（與遊戲或其他模組衝突）');
    }
    const groups = Object.values(enabled.reduce((acc, m) => ((acc[m.id] ??= []).push(m), acc), {}));
    const dupes = groups.filter((g) => g.length > 1);
    for (const group of dupes) {
        for (const mod of [...group].sort(newer).slice(1)) disable(mod, `（重複的舊版本 ${mod.version || mod.file}）`);
    }
    if (dupes.length && !findings.some((f) => f.title === '有重複的模組')) {
        findings.push({ title: '有重複的模組', detail: `${dupes.map((g) => g[0].name).join('、')} 同時裝了不只一個版本，Fabric 會拒絕載入。一鍵修復會保留最新版、停用舊的。` });
    }
    if (report && !fixes.some((f) => ['disable', 'update', 'install'].includes(f.kind))) {
        const suspect = enabled.filter((m) => m.blame > 0 && !m.managed).sort((a, b) => b.blame - a.blame)[0];
        if (suspect) {
            findings.push({ title: `當機報告指向「${suspect.name}」`, detail: `當機時的程式呼叫堆疊裡出現這個模組的程式碼，它很可能就是元兇。可以先停用它再試一次，確認沒問題後再到實例頁的「模組與資源」找更新版本。` });
            disable(suspect, '（當機報告指向它）');
        }
    }
    if (/ZipException|zip END header not found|invalid LOC header|error in opening zip file|Could not find or load main class|Failed to read accessWidener|Unable to read file|corrupt/i.test(text)) {
        add({ kind: 'repair', target: '', label: '驗證並重新下載損壞的遊戲檔案' });
    }
    const gpu = GPU_DLL.exec(native);
    if (gpu) findings.push({ title: '顯示卡驅動程式崩潰', detail: `Java 在顯示卡驅動（${gpu[0]}）裡當機了。請到顯示卡官網更新驅動程式；若使用光影，可先換成較輕量的光影或關閉。` });
    else if (native && !findings.length) findings.push({ title: 'Java 虛擬機崩潰', detail: 'Java 本身意外結束，常見原因是驅動程式、防毒軟體或記憶體設定。請先更新顯示卡驅動、調低記憶體上限再試。' });
    if ((gpu || /GLFW error 65542|WGL: The driver does not appear|Pixel format not accelerated|OpenGL [0-9.]+ is not supported/.test(text)) && !preferDgpu) {
        add({ kind: 'dgpu', target: '', label: '改用獨立顯示卡執行遊戲' });
    }
    return { findings, exception, fixes };
}
