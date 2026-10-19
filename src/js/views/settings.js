import { api, pickFiles } from '../api.js';
import { $, clear, confirm, fail, fmtBytes, h, rangeFill, toast, guard } from '../ui.js';
import { on, saveSettings, store } from '../store.js';
import { gpuName, isIntegratedGpu } from '../services.js';

const FIELDS = {
    memoryMb: '#set-memory',
    width: '#set-width',
    height: '#set-height',
    fullscreen: '#set-fullscreen',
    jvmArgs: '#set-jvm',
    quickJoin: '#set-quick-join',
    javaPath: '#set-java',
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
    closeToTray: '#set-tray',
    autostart: '#set-autostart',
    downloadThreads: '#set-threads',
    notifications: '#set-notify',
    inheritOptions: '#set-inherit',
    musicVolume: '#set-volume'
};

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
        if (el.type === 'checkbox') patch[key] = el.checked;
        else if (el.type === 'number' || el.type === 'range') patch[key] = Number(el.value) || 0;
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

export function initSettings() {
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
