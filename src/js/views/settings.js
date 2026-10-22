import { api, openUrl, pickFiles } from '../api.js';
import { $, clampNumber, clear, confirm, fail, fmtBytes, h, rangeFill, toast, guard } from '../ui.js';
import { emit, loadInstances, loadSettings, on, saveSettings, store } from '../store.js';
import { gpuName, isIntegratedGpu } from '../services.js';

const FIELDS = {
    memoryMb: '#set-memory',
    width: '#set-width',
    height: '#set-height',
    fullscreen: '#set-fullscreen',
    jvmArgs: '#set-jvm',
    quickJoin: '#set-quick-join',
    javaPath: '#set-java',
    javaAutoUpdate: '#set-java-auto',
    useMirror: '#set-mirror',
    showSnapshots: '#set-snapshots',
    showOld: '#set-old',
    injectRichPresence: '#set-rpc',
    afterLaunch: '#set-after',
    backupOnLaunch: '#set-backup',
    backupKeep: '#set-backup-keep',
    preLaunchCmd: '#set-pre',
    postExitCmd: '#set-post',
    slideSeconds: '#set-slide',
    uiScale: '#set-scale',
    reduceMotion: '#set-motion',
    walkers: '#set-walkers',
    closeToTray: '#set-tray',
    autostart: '#set-autostart',
    downloadThreads: '#set-threads',
    notifications: '#set-notify',
    usageStats: '#set-usage',
    inheritOptions: '#set-inherit',
    musicVolume: '#set-volume',
    memoryMinMb: '#set-memory-min',
    wrapper: '#set-wrapper',
    envVars: '#set-env',
    preferDgpu: '#set-dgpu',
    modChannel: '#set-mod-channel',
    autoUpdateMods: '#set-mod-auto',
    proxy: '#set-proxy',
    debugMode: '#set-debug',
    openLogOnLaunch: '#set-open-log',
    launcherUpdate: '#set-launcher-update',
    updateChannel: '#set-update-channel',
    modUpdate: '#set-mod-update'
};

export const JVM_PRESETS = [
    ['G1 調校（預設）', '平衡穩定，適合大多數電腦', '-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M -XX:-UseAdaptiveSizePolicy -XX:-OmitStackTraceInFastThrow'],
    ["Aikar's Flags", '社群常用的 G1 調校，記憶體 6 GB 以上效果較好', '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1'],
    ['分代式 ZGC', '幾乎沒有 GC 停頓，需要 Java 21 以上，記憶體 8 GB 以上較適合', '-XX:+UseZGC -XX:+ZGenerational -XX:-OmitStackTraceInFastThrow']
];

export function jvmPresetButtons(target, onPick) {
    return JVM_PRESETS.map(([label, hint, args]) => h('button', {
        type: 'button',
        class: 'mc',
        title: hint,
        onclick: () => {
            target.value = args;
            onPick();
        }
    }, label));
}

const PROXY_PATTERN = /^https?:\/\/([^\s@/]+@)?[A-Za-z0-9.\-[\]:_]+:\d{1,5}\/?$/;
const SHARE_LABELS = [['saves', '存檔'], ['resourcepacks', '資源包'], ['shaderpacks', '光影'], ['screenshots', '截圖']];

const ACCENTS = [
    ['', '官網配色'],
    ['#2ECC71', '翡翠綠'],
    ['#4dabf7', '天空藍'],
    ['#b197fc', '紫水晶'],
    ['#f783ac', '櫻花粉'],
    ['#ff6b6b', '紅石'],
    ['#ffa94d', '橘子'],
    ['#fcc419', '金塊']
];

const LABELS = {
    '#set-memory': (v) => v,
    '#set-slide': (v) => (Number(v) ? `${v} 秒` : '不輪播'),
    '#set-scale': (v) => `${v}%`,
    '#set-threads': (v) => `${v} 個`,
    '#set-volume': (v) => (Number(v) ? `${v}%` : '靜音')
};

let javaReady = false;

async function fillJavaOptions(current) {
    const select = clear($('#set-java'));
    select.append(h('option', { value: '' }, '自動（推薦）'));
    let list = [];
    try {
        list = await api.list_java();
    } catch (error) {
        fail(error);
    }
    for (const java of list) {
        select.append(h('option', { value: java.path },
            `Java ${java.version}${java.source === 'mojang' ? '（啟動器下載）' : ''} — ${java.path}`));
    }
    if (current && !list.some((j) => j.path === current)) {
        select.append(h('option', { value: current }, `${current}（目前設定）`));
    }
    select.value = current ?? '';
    javaReady = true;
}

function syncLabels() {
    for (const [sel, fmt] of Object.entries(LABELS)) {
        const el = $(sel);
        $(`${sel}-value`).textContent = fmt(el.value);
        rangeFill(el);
    }
}

function read() {
    const patch = {};
    for (const [key, sel] of Object.entries(FIELDS)) {
        const el = $(sel);
        if (key === 'javaPath' && !javaReady) continue;
        if (key === 'proxy' && el.value.trim() && !PROXY_PATTERN.test(el.value.trim())) continue;
        if (el.type === 'checkbox') patch[key] = el.checked;
        else if (el.type === 'number' || el.type === 'range') patch[key] = clampNumber(el);
        else patch[key] = el.value;
    }
    patch.backupKeep = Math.max(1, patch.backupKeep);
    return patch;
}

function renderAccent() {
    const current = (store.settings?.accent ?? '').toLowerCase();
    const custom = current && !ACCENTS.some(([value]) => value.toLowerCase() === current);
    const picker = h('input', {
        type: 'color',
        value: custom ? current : '#8491ff',
        onchange: (e) => persist({ accent: e.target.value })
    });
    clear($('#set-accent')).append(
        ...ACCENTS.map(([value, label]) => h('button', {
            type: 'button',
            class: `swatch${value ? '' : ' site'}`,
            style: value ? `--swatch:${value}` : '',
            title: label,
            'aria-label': label,
            'aria-pressed': String(value.toLowerCase() === current),
            onclick: () => persist({ accent: value })
        })),
        h('label', { class: 'swatch custom', title: '自訂顏色', 'aria-pressed': String(!!custom), style: custom ? `--swatch:${current}` : '' }, picker)
    );
}

function write() {
    const settings = store.settings;
    if (!settings) return;
    for (const [key, sel] of Object.entries(FIELDS)) {
        const el = $(sel);
        if (el === $('#set-java') || el === document.activeElement) continue;
        if (el.type === 'checkbox') el.checked = !!settings[key];
        else el.value = settings[key] ?? '';
    }
    $('#set-backup-keep').disabled = !settings.backupOnLaunch;
    const shared = settings.sharedFolders ?? [];
    for (const box of $('#set-shared').querySelectorAll('input')) box.checked = shared.includes(box.value);
    $('#bg-state').textContent = !settings.background ? '目前使用官網輪播' : /\.(mp4|webm)$/i.test(settings.background) ? '目前使用自訂影片' : '目前使用自訂圖片';
    $('#music-state').textContent = settings.music ? `目前播放：${settings.music.split(/[\\/]/).pop().replace(/^music-\d+\./, '自訂音樂.')}` : '未設定';
    $('#btn-music-clear').disabled = !settings.music;
    $('#set-slide').disabled = !!settings.background;
    syncLabels();
    renderAccent();
}

let savedTimer = 0;
async function persist(patch) {
    const state = $('#save-state');
    try {
        await saveSettings(patch);
        store.versions = null;
        state.textContent = '已儲存';
        state.classList.add('saved');
        clearTimeout(savedTimer);
        savedTimer = setTimeout(() => {
            state.textContent = '變更會自動儲存';
            state.classList.remove('saved');
        }, 1500);
    } catch (error) {
        fail(error);
    }
}

let manualCheck = false;

function renderUpdate() {
    const box = clear($('#update-status'));
    const u = store.update;
    if (!u) {
        box.append(h('div', { class: 'empty-state' }, store.settings?.launcherUpdate === 'off' ? '已關閉自動檢查，按「立即檢查更新」查看有沒有新版。' : '正在檢查更新⋯'));
        return;
    }
    const row = (label, value) => h('div', { class: 'row' }, h('div', { class: 'grow' }, h('div', { class: 'name' }, label), h('div', { class: 'meta' }, value)));
    const latest = u.latest ? `v${u.latest.version}${u.newer ? '（可以更新）' : '（已是最新）'}` : u.reachable ? '—' : '暫時連不上官網';
    const signed = !u.latest ? '—' : !u.self ? '這個平台請到官網下載新版' : u.signed ? '已附輝煌官方數位簽章，安裝前會先驗證' : '這個版本沒有數位簽章，啟動器不會自動安裝';
    box.append(row('目前版本', `v${u.current}`), row('最新版本', latest), row('數位簽章', signed),
        row('上次檢查', new Date(u.checkedAt).toLocaleString('zh-TW', { hour12: false })));
    if (manualCheck) {
        manualCheck = false;
        toast(!u.reachable ? '暫時連不上官網，請稍後再試' : u.newer ? `有新版 v${u.latest.version}，可以點右上角的更新按鈕` : '已經是最新版本', u.reachable ? '' : 'warn');
    }
}

export function initSettings() {
    on('update-status', renderUpdate);
    const syncAlpha = () => { $('#opt-alpha').hidden = !store.settings?.alphaUnlocked && store.settings?.updateChannel !== 'alpha'; };
    on('settings', syncAlpha);
    syncAlpha();
    let lastChannel = store.settings?.updateChannel || 'stable';
    on('settings', () => {
        const channel = store.settings?.updateChannel || 'stable';
        if (channel === lastChannel) return;
        lastChannel = channel;
        toast(channel === 'stable' ? '已切到正式版通道，目前的版本會用到下一個正式版推出為止' : `已切到 ${channel === 'beta' ? 'Beta 搶先版' : 'Alpha 測試版'}通道，正在檢查更新`);
        emit('check-update');
    });
    on('settings', () => { if (!store.update) renderUpdate(); });
    $('#btn-check-update').addEventListener('click', () => {
        manualCheck = true;
        emit('check-update');
    });
    $('#btn-update-page').addEventListener('click', () => openUrl('https://www.brilliantw.net/launcher/'));
    const view = $('#view-settings');
    const isSetting = (el) => Object.values(FIELDS).some((sel) => el.matches(sel));
    let typing = 0;
    view.addEventListener('input', (e) => {
        if (!isSetting(e.target)) return;
        syncLabels();
        clearTimeout(typing);
        typing = setTimeout(() => persist(read()), e.target.type === 'range' ? 250 : 600);
    });
    view.addEventListener('change', (e) => {
        if (!isSetting(e.target)) return;
        clearTimeout(typing);
        persist(read());
    });

    $('#settings-nav').addEventListener('click', (e) => {
        const link = e.target.closest('[data-jump]');
        if (!link) return;
        e.preventDefault();
        $(`#${link.dataset.jump}`).scrollIntoView({ behavior: 'smooth', block: 'start' });
    });

    $('#btn-bg-pick').addEventListener('click', guard(async () => {
        const path = await pickFiles({ multiple: false, filters: [{ name: '圖片或影片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'mp4', 'webm'] }] }).catch(fail);
        if (!path) return;
        try {
            await persist({ background: await api.set_media({ kind: 'background', path }) });
        } catch (error) {
            fail(error);
        }
    }));
    $('#btn-music-pick').addEventListener('click', guard(async () => {
        const path = await pickFiles({ multiple: false, filters: [{ name: '音樂', extensions: ['mp3', 'ogg', 'wav', 'm4a', 'flac', 'opus'] }] }).catch(fail);
        if (!path) return;
        try {
            await persist({ music: await api.set_media({ kind: 'music', path }) });
        } catch (error) {
            fail(error);
        }
    }));
    $('#btn-music-clear').addEventListener('click', guard(() => persist({ music: '' })));
    $('#btn-bg-reset').addEventListener('click', guard(() => persist({ background: '' })));

    $('#jvm-presets').append(...jvmPresetButtons($('#set-jvm'), () => {
        persist(read());
        toast('已套用 JVM 參數預設組');
    }));
    $('#set-shared').append(...SHARE_LABELS.map(([value, label]) => h('label', {},
        h('input', {
            type: 'checkbox',
            value,
            onchange: () => persist({ sharedFolders: [...$('#set-shared').querySelectorAll('input:checked')].map((b) => b.value) })
        }), label)));
    $('#btn-config-export').addEventListener('click', guard(async () => {
        try {
            const path = await api.export_config();
            toast(`已匯出設定到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
            api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
        } catch (error) {
            fail(error);
        }
    }));
    $('#btn-config-import').addEventListener('click', guard(async () => {
        const path = await pickFiles({ multiple: false, filters: [{ name: '輝煌啟動器設定', extensions: ['zip'] }] }).catch(fail);
        if (!path) return;
        if (!(await confirm('匯入會用檔案裡的啟動器設定取代目前的設定，並加入目前沒有的實例（已存在的實例不會被動到）。要繼續嗎？', '匯入'))) return;
        try {
            const result = await api.import_config({ path });
            await loadSettings();
            emit('settings');
            await loadInstances().catch(() => {});
            toast(`匯入完成：${result.settings ? '已套用啟動器設定，' : ''}新增 ${result.added} 個實例${result.skipped ? `，略過 ${result.skipped} 個已存在的實例` : ''}`);
            for (const note of result.notes ?? []) toast(note, 'warn');
        } catch (error) {
            fail(error);
        }
    }));

    $('#btn-open-root').addEventListener('click', guard(async () => {
        const paths = await api.launcher_paths();
        await api.open_path({ path: paths[0][1] }).catch(fail);
    }));
    $('#btn-clean-cache').addEventListener('click', guard(async () => {
        if (!(await confirm('要清理快取嗎？存檔、模組與設定都不會受影響。', '清理'))) return;
        try {
            toast(`已清理 ${fmtBytes(await api.clean_cache())}`);
        } catch (error) {
            fail(error);
        }
    }));

    api.system_info().then((info) => {
        const gpu = gpuName();
        const integrated = isIntegratedGpu(gpu);
        clear($('#hardware')).append(
            h('div', { class: 'grow' },
                h('b', {}, '你的電腦'),
                h('small', {}, `記憶體 ${Math.round(info.memoryMb / 1024)} GB｜${info.cpus} 執行緒｜${gpu || '顯示卡未知'}`),
                h('small', {}, `建議遊戲記憶體 ${info.recommendedMb} MB${integrated ? '；內建顯示卡建議安裝「效能優化包」，不建議開光影' : ''}`)),
            h('button', {
                type: 'button',
                class: 'mc',
                onclick: () => persist({ memoryMb: info.recommendedMb })
            }, '套用建議'));
    }).catch(() => {});

    on('settings', write);
    write();
    fillJavaOptions(store.settings?.javaPath);

    api.launcher_paths().then((paths) => {
        clear($('#about-paths')).append(
            ...paths.map(([label, path]) =>
                h('div', { class: 'row' },
                    h('div', { class: 'grow' },
                        h('div', { class: 'name' }, label),
                        h('div', { class: 'meta' }, path)),
                    h('button', {
                        type: 'button',
                        class: 'mc',
                        onclick: () => api.open_path({ path }).catch(fail)
                    }, '開啟'))
            )
        );
    }).catch(() => {});
}
