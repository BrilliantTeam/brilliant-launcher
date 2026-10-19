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
            return `Fabric 拒絕載入目前的模組組合。${reasons.length ? `\n${reasons.map((r) => `・${r}`).join('\n')}` : ''}\n請到「模組」頁安裝缺少的前置模組，或移除不支援這個版本的模組。`;
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
        detail: () => '同一個模組被放了兩份（常見於不同版本的同名 jar）。請到「模組」頁刪掉舊的那一份。'
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
