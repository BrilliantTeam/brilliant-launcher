import { api, openUrl, pickFile } from '../api.js';
import { $, $$, acceptFiles, clampNumber, clear, confirm, dialog, fail, fmtBytes, h, pickColor, rangeFill, toast, guard } from '../ui.js';
import { emit, loadInstances, loadSettings, on, saveSettings, store } from '../store.js';
import { icon } from '../icons.js';
import { gpuName, isIntegratedGpu } from '../services.js';
import { SHARDS } from './home.js';
import { playClick } from '../sound.js';

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
    modUpdate: '#set-mod-update',
    buttonStyle: '#set-buttons',
    corners: '#set-corners',
    cardSize: '#set-cards',
    panelOpacity: '#set-panel',
    bgDim: '#set-dim',
    railLabels: '#set-rail-labels',
    showNews: '#set-news',
    contentWidth: '#set-content-width',
    density: '#set-density',
    accentBar: '#set-accent-bar',
    bgBlur: '#set-blur',
    bgMotion: '#set-bg-motion',
    thinScrollbars: '#set-thin-scroll',
    cardDetails: '#set-card-details',
    showBrand: '#set-brand',
    homeTagline: '#set-tagline',
    railStyle: '#set-rail-style',
    coverHeight: '#set-cover-height',
    cursorStyle: '#set-cursor',
    bgFilter: '#set-bg-filter',
    homeLayout: '#set-home-layout',
    showClock: '#set-clock',
    clickSound: '#set-click-sound',
    theme: '#set-theme',
    darkFrom: '#set-dark-from',
    darkTo: '#set-dark-to',
    accentFromBg: '#set-accent-bg',
    highContrast: '#set-contrast',
    font: '#set-font',
    headingStyle: '#set-heading',
    fontScale: '#set-font-scale',
    bgSource: '#set-bg-source',
    bgTransition: '#set-bg-transition',
    bgPan: '#set-bg-pan',
    weather: '#set-weather',
    shardLayout: '#set-shard-layout',
    playSize: '#set-play-size',
    playText: '#set-play-text',
    showCredit: '#set-credit',
    showSocial: '#set-social',
    showShardStats: '#set-shard-stats',
    showLastPlayed: '#set-last-played',
    clockFormat: '#set-clock-format',
    clockStyle: '#set-clock-style',
    clockPosition: '#set-clock-position',
    clockSeconds: '#set-clock-seconds',
    cardView: '#set-card-view',
    instanceSort: '#set-inst-sort',
    cardHover: '#set-card-hover',
    coverPosition: '#set-cover-pos',
    coverShade: '#set-cover-shade',
    pageStats: '#set-page-stats',
    railSide: '#set-rail-side',
    startPage: '#set-start-page',
    animSpeed: '#set-anim-speed',
    pageTransition: '#set-page-transition',
    clickSoundType: '#set-click-type',
    clickVolume: '#set-click-volume',
    toastPosition: '#set-toast-pos',
    toastSeconds: '#set-toast-seconds',
    launchSplash: '#set-launch-splash'
};

const LOOK_PRESETS = [
    ['輝煌經典', 'Minecraft 方塊按鈕、金色邊條，原汁原味', {}],
    ['現代玻璃', '扁平按鈕、圓角、霧化面板與緩慢縮放的背景', { buttonStyle: 'flat', corners: 'rounded', panelOpacity: 75, bgDim: 20, bgBlur: 4, bgMotion: true, accentBar: 'thin' }],
    ['極簡', '收起標誌與公告，只留下必要的資訊', { buttonStyle: 'flat', corners: 'rounded', accentBar: 'none', showNews: false, showBrand: false, railLabels: false, cardDetails: false }],
    ['緊湊效率', '寬版、緊湊間距，一次看到更多實例與模組', { contentWidth: 'wide', density: 'compact', cardSize: 'compact', railLabels: false, thinScrollbars: true, coverHeight: 'short' }],
    ['像素懷舊', '方塊按鈕、懷舊色調、像素游標與點擊音效', { bgFilter: 'sepia', cursorStyle: 'pixel', clickSound: true, railStyle: 'solid', coverHeight: 'tall', bgDim: 15 }]
];

const LOOK_DEFAULTS = {
    accent: '', buttonStyle: 'pixel', corners: 'square', cardSize: 'comfy', panelOpacity: 100, bgDim: 0, railLabels: true, showNews: true, uiScale: 100, reduceMotion: false,
    contentWidth: 'standard', density: 'comfy', accentBar: 'thick', bgBlur: 0, bgMotion: false, thinScrollbars: false, cardDetails: true, showBrand: true, homeTagline: '',
    railStyle: 'glass', coverHeight: 'normal', cursorStyle: 'system', bgFilter: 'none', homeLayout: 'standard', showClock: false, clickSound: false,
    theme: 'auto', darkFrom: 19, darkTo: 7, accentFromBg: false, colorSuccess: '', colorWarning: '', colorLink: '', highContrast: false,
    font: 'pixel', headingStyle: 'game', fontScale: 100, slideSeconds: 15,
    bgSource: 'auto', bgColor: '', bgColor2: '', bgPin: '', bgTransition: 'fade', bgPan: false, weather: 'none',
    shardLayout: 'grid', hiddenShards: [], playSize: 'normal', playText: '', showCredit: true, showSocial: true, showShardStats: true, showLastPlayed: true,
    clockFormat: '24', clockStyle: 'bold', clockPosition: 'right', clockSeconds: false,
    cardView: 'grid', instanceSort: 'manual', cardHover: 'lift', coverPosition: 'center', coverShade: 35, pageStats: true,
    railSide: 'left', railOrder: [], railHidden: [], hideAccountName: false, hiddenTools: [], startPage: 'home',
    animSpeed: 'normal', pageTransition: 'rise', clickSoundType: 'block', clickVolume: 50, toastPosition: 'bottom-right', toastSeconds: 6, launchSplash: false
};

const STYLE_KEYS = ['buttonStyle', 'corners', 'cardSize', 'panelOpacity', 'bgDim', 'railLabels', 'showNews', 'contentWidth', 'density', 'accentBar', 'bgBlur', 'bgMotion',
    'thinScrollbars', 'cardDetails', 'showBrand', 'railStyle', 'coverHeight', 'cursorStyle', 'bgFilter', 'homeLayout', 'showClock', 'clickSound', 'accentFromBg', 'highContrast',
    'headingStyle', 'bgTransition', 'bgPan', 'weather', 'shardLayout', 'playSize', 'clockStyle', 'clockPosition', 'cardView', 'cardHover', 'coverPosition', 'coverShade',
    'pageTransition', 'clickSoundType', 'launchSplash'];

const LOOK_LOCAL = ['bgPin', 'railOrder', 'railHidden', 'hiddenTools', 'hiddenShards', 'startPage', 'hideAccountName', 'uiScale', 'reduceMotion'];

function cleanLook(raw) {
    const look = {};
    for (const [key, value] of Object.entries(raw && typeof raw === 'object' ? raw : {})) {
        const fallback = LOOK_DEFAULTS[key];
        if (fallback === undefined) continue;
        if (Array.isArray(fallback) ? Array.isArray(value) && value.every((v) => typeof v === 'string') : typeof value === typeof fallback) look[key] = value;
    }
    return look;
}

async function applyLook(raw, name) {
    const base = Object.fromEntries(STYLE_KEYS.map((key) => [key, LOOK_DEFAULTS[key]]));
    await persist({ ...base, ...cleanLook(raw) });
    toast(`已套用「${name}」風格`);
}

const lookCard = (name, hint, look, by = '') => h('button', { type: 'button', class: 'look-preset', onclick: () => applyLook(look, name) },
    h('b', {}, name, by ? h('span', { class: 'look-author' }, by) : null), h('small', {}, hint));

function renderLookPresets() {
    clear($('#look-presets')).append(...LOOK_PRESETS.map(([name, hint, patch]) => lookCard(name, hint, patch)));
}

async function renderCommunityLooks() {
    const box = $('#look-community');
    const data = await api.site_json({ path: '/api/launcher/looks' }).catch(() => null);
    const looks = Array.isArray(data?.looks) ? data.looks.slice(0, 30) : null;
    clear(box).append(...(looks?.length
        ? looks.map((item) => lookCard(String(item.name).slice(0, 24), String(item.description ?? '').slice(0, 80), item.look, String(item.author ?? '').slice(0, 32)))
        : [h('div', { class: 'hint' }, looks ? '目前還沒有社群風格。' : '暫時連不上官網，稍後再回來看看。')]));
}

const LOOK_CODE = 'BRLOOK1:';

function exportLook() {
    const settings = store.settings ?? {};
    const look = Object.fromEntries(Object.keys(LOOK_DEFAULTS).filter((key) => !LOOK_LOCAL.includes(key) && settings[key] !== undefined).map((key) => [key, settings[key]]));
    const bytes = new TextEncoder().encode(JSON.stringify(look));
    const code = LOOK_CODE + btoa(String.fromCharCode(...bytes));
    const text = h('textarea', { readonly: true, spellcheck: 'false', class: 'look-code' }, code);
    navigator.clipboard?.writeText(code).then(() => toast('已複製外觀代碼'), () => {});
    dialog({
        title: '分享我的外觀',
        content: h('div', { class: 'field' },
            h('label', {}, '外觀代碼（已複製）'), text,
            h('div', { class: 'hint' }, '把代碼貼給朋友，對方在「匯入外觀」貼上就能套用；也可以分享到輝煌 Discord，精選作品會上架到「社群風格」。背景圖片、音樂與側欄排序不會包含在代碼裡。')),
        buttons: [{ label: '關閉', value: null }]
    });
    text.select();
}

async function importLook() {
    const text = h('textarea', { spellcheck: 'false', class: 'look-code', placeholder: `${LOOK_CODE}⋯` });
    const ok = await dialog({
        title: '匯入外觀',
        content: h('div', { class: 'field' }, h('label', {}, '貼上外觀代碼'), text),
        buttons: [{ label: '取消', value: false }, { label: '套用', value: true }]
    });
    if (ok !== true) return;
    const code = text.value.trim();
    let look = null;
    try {
        if (!code.startsWith(LOOK_CODE)) throw new Error();
        const bytes = Uint8Array.from(atob(code.slice(LOOK_CODE.length)), (c) => c.charCodeAt(0));
        look = cleanLook(JSON.parse(new TextDecoder().decode(bytes)));
    } catch {
        return toast(`這不是有效的外觀代碼，代碼應該以「${LOOK_CODE}」開頭`, 'error');
    }
    if (!Object.keys(look).length) return toast('代碼裡沒有可以套用的外觀設定', 'warn');
    await persist(look);
    toast('已匯入外觀');
}

function renderColorFields() {
    for (const field of $$('.color-field')) {
        const key = field.dataset.colorKey;
        const value = store.settings?.[key] ?? '';
        const label = field.dataset.colorLabel;
        const swatch = h('button', {
            type: 'button',
            class: `swatch${value ? '' : ' empty'}`,
            style: value ? `--swatch:${value}` : '',
            title: `選擇${label}`,
            'aria-label': `選擇${label}`,
            onclick: (e) => pickColor(e.currentTarget, value || '#8491ff', (v) => persist({ [key]: v }), label)
        });
        clear(field).append(swatch, h('span', {}, label, h('small', {}, value || '預設')),
            value ? h('button', { type: 'button', class: 'link', onclick: () => persist({ [key]: '' }) }, '清除') : null);
    }
}

function renderShardVisibility() {
    const hidden = store.settings?.hiddenShards ?? [];
    clear($('#shard-visibility')).append(...SHARDS.map((shard) => h('label', {},
        h('input', {
            type: 'checkbox',
            checked: !hidden.includes(shard.host),
            onchange: (e) => persist({ hiddenShards: e.target.checked ? hidden.filter((x) => x !== shard.host) : [...hidden, shard.host] })
        }), shard.label)));
}

const RAIL_LOCKED = ['home', 'instances', 'settings'];

export function railItems() {
    const links = $$('.rail a[data-view]').filter((a) => a.dataset.view !== 'about');
    const order = store.settings?.railOrder ?? [];
    const rank = (view) => (order.includes(view) ? order.indexOf(view) : order.length + links.findIndex((a) => a.dataset.view === view));
    return links.map((a) => a.dataset.view).sort((a, b) => rank(a) - rank(b));
}

function renderRailEditor() {
    const items = railItems();
    const hidden = store.settings?.railHidden ?? [];
    const move = (i, step) => {
        const next = [...items];
        [next[i], next[i + step]] = [next[i + step], next[i]];
        persist({ railOrder: next });
    };
    clear($('#rail-editor')).append(...items.map((view, i) => {
        const link = $(`.rail a[data-view="${view}"]`);
        return h('div', { class: 'rail-item' },
            h('label', {},
                h('input', {
                    type: 'checkbox',
                    checked: !hidden.includes(view),
                    disabled: RAIL_LOCKED.includes(view),
                    onchange: (e) => persist({ railHidden: e.target.checked ? hidden.filter((x) => x !== view) : [...hidden, view] })
                }),
                link.querySelector('svg')?.cloneNode(true), link.title),
            h('button', { type: 'button', class: 'mc icon-btn', title: '往上移', 'aria-label': `${link.title}往上移`, disabled: i === 0, onclick: () => move(i, -1) }, icon('arrow-up')),
            h('button', { type: 'button', class: 'mc icon-btn', title: '往下移', 'aria-label': `${link.title}往下移`, disabled: i === items.length - 1, onclick: () => move(i, 1) }, icon('arrow-down')));
    }));
}

let heroSlides = null;

async function renderBgPins() {
    const box = $('#bg-pins');
    heroSlides ??= await api.site_json({ path: '/api/launcher/hero' }).then((d) => d?.slides ?? [], () => []);
    const pin = store.settings?.bgPin ?? '';
    clear(box).append(
        h('button', { type: 'button', class: 'bg-pin random', 'aria-pressed': String(!pin), onclick: () => persist({ bgPin: '' }) }, icon('reload'), '隨機輪播'),
        ...heroSlides.map((slide) => h('button', {
            type: 'button',
            class: 'bg-pin',
            title: [slide.description, slide.author && `攝影：${slide.author}`].filter(Boolean).join('｜'),
            'aria-label': slide.description || '背景圖片',
            'aria-pressed': String(pin === slide.image),
            style: `background-image:url("${slide.image.replace(/&w=\d+$/, '&w=320')}")`,
            onclick: () => persist({ bgPin: slide.image })
        })));
}

function syncConditions() {
    const settings = store.settings ?? {};
    for (const el of $$('#view-settings [data-when]')) {
        const [key, want] = el.dataset.when.split('=');
        const value = settings[key] ?? LOOK_DEFAULTS[key];
        el.style.display = (want === undefined ? !!value : String(value) === want) ? '' : 'none';
    }
}

export const JVM_PRESETS = [
    ['G1 調校（預設）', '平衡穩定，適合大多數電腦', '-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M -XX:-UseAdaptiveSizePolicy -XX:-OmitStackTraceInFastThrow'],
    ["Aikar's Flags", '社群常用的 G1 調校，記憶體 6 GB 以上效果較好', '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1'],
    ['分代式 ZGC', '幾乎沒有 GC 停頓，需要 Java 21 以上，記憶體 8 GB 以上較適合（Java 23 起分代模式是預設，啟動器會自動略過已移除的參數）', '-XX:+UseZGC -XX:+ZGenerational -XX:-OmitStackTraceInFastThrow']
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
export const SHARE_LABELS = [['saves', '存檔'], ['resourcepacks', '資源包'], ['shaderpacks', '光影'], ['screenshots', '截圖']];
export const SHARE_INFO = {
    saves: ['map', '在任一實例建立或下載的單人世界，其他實例也看得到。只建議同一個 Minecraft 版本的實例共用，高版本開過的世界會被升級。'],
    resourcepacks: ['image', '在任一實例加入的資源包，其他實例也看得到；啟用與排序仍由各實例自己決定。'],
    shaderpacks: ['eye', '光影包只要放一份，所有參與的實例都能選用。'],
    screenshots: ['image', '所有參與實例的截圖集中在同一個資料夾，相簿也會一起顯示。']
};

function renderSharedSummary() {
    const box = $('#set-shared');
    if (!box) return;
    const groups = (store.settings?.instanceGroups ?? []).filter((g) => g.shared?.length);
    clear(box).append(
        h('span', { class: 'share-summary' }, groups.length
            ? groups.map((g) => `「${g.name}」共用 ${g.shared.map((f) => SHARE_LABELS.find(([v]) => v === f)?.[1]).filter(Boolean).join('、')}`).join('；')
            : '共用資料夾改在實例分組中設定：每個分組可以各自選擇要共用的資料夾。'),
        h('button', { type: 'button', class: 'mc', onclick: () => { location.hash = '#instances'; } }, icon('folder'), '前往實例分組'));
}

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
    '#set-slide': (v) => (!Number(v) ? '不輪播' : v >= 60 ? `${Math.floor(v / 60)} 分${v % 60 ? ` ${v % 60} 秒` : '鐘'}` : `${v} 秒`),
    '#set-scale': (v) => `${v}%`,
    '#set-threads': (v) => `${v} 個`,
    '#set-volume': (v) => (Number(v) ? `${v}%` : '靜音'),
    '#set-panel': (v) => `${v}%`,
    '#set-dim': (v) => (Number(v) ? `${v}%` : '不暗化'),
    '#set-blur': (v) => (Number(v) ? `${v}` : '不模糊'),
    '#set-font-scale': (v) => `${v}%`,
    '#set-cover-shade': (v) => (Number(v) ? `${v}%` : '不加遮罩'),
    '#set-click-volume': (v) => (Number(v) ? `${v}%` : '靜音'),
    '#set-toast-seconds': (v) => `${v} 秒`
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
        else if (el.classList.contains('hour-select')) patch[key] = Number(el.value);
        else patch[key] = el.value;
    }
    patch.backupKeep = Math.max(1, patch.backupKeep);
    return patch;
}

function renderAccent() {
    const current = (store.settings?.accent ?? '').toLowerCase();
    const custom = current && !ACCENTS.some(([value]) => value.toLowerCase() === current);
    const swatch = ([value, label]) => h('button', {
        type: 'button',
        class: `swatch${value ? '' : ' site'}`,
        style: value ? `--swatch:${value}` : '',
        title: label,
        'aria-label': label,
        'aria-pressed': String(value.toLowerCase() === current),
        onclick: () => persist({ accent: value })
    });
    clear($('#set-accent')).append(
        ...ACCENTS.map(swatch),
        h('button', {
            type: 'button',
            class: 'swatch custom',
            title: '自訂顏色',
            'aria-label': '自訂顏色',
            'aria-pressed': String(!!custom),
            'aria-haspopup': 'dialog',
            style: custom ? `--swatch:${current}` : '',
            onclick: (e) => pickColor(e.currentTarget, custom ? current : '#8491ff', (value) => persist({ accent: value }), '自訂主題色')
        })
    );
}

function write() {
    const settings = store.settings;
    if (!settings) return;
    for (const [key, sel] of Object.entries(FIELDS)) {
        const el = $(sel);
        if (el === $('#set-java') || el === document.activeElement) continue;
        const value = settings[key] ?? LOOK_DEFAULTS[key];
        if (el.type === 'checkbox') el.checked = !!value;
        else el.value = value ?? '';
    }
    $('#set-backup-keep').disabled = !settings.backupOnLaunch;
    renderSharedSummary();
    $('#bg-state').textContent = !settings.background ? '目前使用官網輪播' : /\.(mp4|webm)$/i.test(settings.background) ? '目前使用自訂影片' : '目前使用自訂圖片';
    $('#music-state').textContent = settings.music ? `目前播放：${settings.music.split(/[\\/]/).pop().replace(/^music-\d+\./, '自訂音樂.')}` : '未設定';
    $('#btn-music-clear').disabled = !settings.music;
    $('#set-slide').disabled = !!settings.background || !!settings.bgPin;
    $('#set-account-name').checked = !settings.hideAccountName;
    for (const box of $$('#tool-visibility [data-tool]')) box.checked = !(settings.hiddenTools ?? []).includes(box.dataset.tool);
    syncLabels();
    syncConditions();
    renderAccent();
    renderColorFields();
    renderShardVisibility();
    renderRailEditor();
    if (heroSlides) renderBgPins();
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

const PAGE_KEY = 'brilliant.settings.page';
const GROUP_KEY = 'brilliant.settings.lookGroup';

function initPages(view) {
    const pages = [...view.querySelectorAll(':scope > h2')].map((title) => {
        const page = h('section', { class: 'settings-page', 'data-page': title.id });
        title.before(page);
        for (let el = title; el && (el === title || el.nodeName !== 'H2') && el.id !== 'settings-empty';) {
            const next = el.nextElementSibling;
            page.append(el);
            el = next;
        }
        return page;
    });
    const links = [...view.querySelectorAll('#settings-nav [data-jump]')];
    const search = $('#settings-search');
    const lookPage = pages.find((p) => p.dataset.page === 'set-look');
    const groupNames = ['風格'];
    for (const block of [...lookPage.children].slice(1)) {
        if (block.matches('h3.look-group')) groupNames.push(block.textContent);
        block.dataset.group = String(groupNames.length - 1);
    }
    let group = '0';
    try {
        group = localStorage.getItem(GROUP_KEY) || '0';
    } catch {}
    if (!groupNames[group]) group = '0';
    const groupTabs = h('div', { class: 'segmented look-tabs', role: 'tablist', 'aria-label': '外觀分類' },
        ...groupNames.map((name, i) => h('button', {
            type: 'button',
            role: 'tab',
            'data-group': String(i),
            onclick: () => {
                group = String(i);
                try {
                    localStorage.setItem(GROUP_KEY, group);
                } catch {}
                render();
                $('.page-wrap').scrollTop = 0;
            }
        }, name)));
    lookPage.querySelector('h2').after(groupTabs);
    let current = '';
    try {
        current = localStorage.getItem(PAGE_KEY) || '';
    } catch {}
    const render = () => {
        const words = search.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
        view.classList.toggle('searching', words.length > 0);
        let found = 0;
        for (const page of pages) {
            const title = page.querySelector('h2').textContent.toLowerCase();
            let hit = false;
            for (const block of page.children) {
                if (block.nodeName === 'H2' || block === groupTabs) continue;
                const text = `${title} ${block.textContent.toLowerCase()}`;
                const match = words.every((w) => text.includes(w));
                block.classList.toggle('search-miss', !match);
                block.classList.toggle('group-off', !words.length && block.dataset.group !== undefined && (block.dataset.group !== group || block.matches('h3.look-group')));
                hit ||= match;
            }
            page.hidden = words.length ? !hit : page.dataset.page !== current;
            if (words.length && hit) found++;
        }
        for (const link of links) link.setAttribute('aria-current', String(!words.length && link.dataset.jump === current));
        groupTabs.hidden = words.length > 0;
        for (const tab of groupTabs.children) tab.setAttribute('aria-selected', String(tab.dataset.group === group));
        $('#settings-empty').hidden = !words.length || found > 0;
    };
    const show = (id) => {
        current = pages.some((p) => p.dataset.page === id) ? id : pages[0].dataset.page;
        try {
            localStorage.setItem(PAGE_KEY, current);
        } catch {}
        search.value = '';
        render();
        view.scrollTop = 0;
    };
    $('#settings-nav').addEventListener('click', (e) => {
        const link = e.target.closest('[data-jump]');
        if (!link) return;
        e.preventDefault();
        show(link.dataset.jump);
    });
    search.addEventListener('input', render);
    search.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && search.value) {
            e.stopPropagation();
            search.value = '';
            render();
        }
    });
    show(current);
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

    for (const select of $$('.hour-select')) select.append(...Array.from({ length: 24 }, (_, hour) => h('option', { value: String(hour) }, `${String(hour).padStart(2, '0')}:00`)));
    initPages(view);
    renderLookPresets();
    renderCommunityLooks();
    renderBgPins();
    $('#btn-look-reset').addEventListener('click', guard(async () => {
        if (!(await confirm('要把主題色、按鈕風格、邊角、字體、側欄、動畫等外觀設定全部還原成預設嗎？\n背景圖片與音樂不會被移除。', '還原'))) return;
        await persist(LOOK_DEFAULTS);
        toast('已還原預設外觀');
    }));
    $('#btn-look-export').addEventListener('click', exportLook);
    $('#btn-look-import').addEventListener('click', guard(importLook));
    $('#set-account-name').addEventListener('change', (e) => persist({ hideAccountName: !e.target.checked }));
    $('#tool-visibility').addEventListener('change', (e) => {
        const tool = e.target.dataset.tool;
        if (!tool) return;
        const hidden = store.settings?.hiddenTools ?? [];
        persist({ hiddenTools: e.target.checked ? hidden.filter((x) => x !== tool) : [...hidden, tool] });
    });
    for (const sel of ['#set-click-type', '#set-click-volume']) {
        $(sel).addEventListener('change', () => playClick($('#set-click-type').value, $('#set-click-volume').value));
    }

    const setMedia = (kind) => async (path) => {
        if (!path) return;
        try {
            await persist({ [kind]: await api.set_media({ kind, path }) });
        } catch (error) {
            fail(error);
        }
    };
    $('#btn-bg-pick').addEventListener('click', guard(async () => setMedia('background')(await pickFile('background').catch(fail))));
    $('#btn-music-pick').addEventListener('click', guard(async () => setMedia('music')(await pickFile('music').catch(fail))));
    acceptFiles($('#btn-bg-pick').parentElement, 'background', setMedia('background'));
    acceptFiles($('#btn-music-pick').parentElement, 'music', setMedia('music'));
    $('#btn-music-clear').addEventListener('click', guard(() => persist({ music: '' })));
    $('#btn-bg-reset').addEventListener('click', guard(() => persist({ background: '' })));

    $('#jvm-presets').append(...jvmPresetButtons($('#set-jvm'), () => {
        persist(read());
        toast('已套用 JVM 參數預設組');
    }));
    on('instances', renderSharedSummary);
    $('#btn-config-export').addEventListener('click', guard(async () => {
        try {
            const path = await api.export_config();
            toast(`已匯出設定到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
            api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
        } catch (error) {
            fail(error);
        }
    }));
    const importConfig = async (path) => {
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
    };
    $('#btn-config-import').addEventListener('click', guard(async () => importConfig(await pickFile('config').catch(fail))));
    acceptFiles($('#btn-config-import'), 'config', importConfig);

    $('#btn-open-root').addEventListener('click', guard(async () => {
        const paths = await api.launcher_paths();
        await api.open_path({ path: paths[0][1] }).catch(fail);
    }));
    $('#btn-clean-cache').addEventListener('click', guard(async () => {
        if (!(await confirm('要清理快取嗎？存檔、模組與設定都不會受影響；回收資料夾裡放超過 30 天的項目會永久刪除。', '清理'))) return;
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
