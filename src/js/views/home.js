import { api, listen, openUrl } from '../api.js';
import { $, clear, confirm, dialog, fail, fmtBytes, h, toast } from '../ui.js';
import { accountFor, avatarUrl, emit, isRunning, loadAccounts, loadInstances, on, selectedAccount, selectedInstance, store, saveSettings } from '../store.js';
import { initNews } from './news.js';
import { icon, instanceIconName, instanceVisual } from '../icons.js';
import { addOffline, loginMicrosoft } from './accounts.js';
import { createGeneralInstance, quickCreate, upgradeFlow, versionData } from './instances.js';
import { notifyOnce, ready, upgradeTarget } from '../services.js';
import { clearRecentLog, recentLog } from './logview.js';
import { plan } from '../crash.js';
import { perfAfterExit } from '../perf.js';
import { reportDialog } from '../report.js';

export const SHARDS = [
    { host: 's1.brilliantw.net', label: '炒飯服' },
    { host: 's2.brilliantw.net', label: '書碼服' },
    { host: 's3.brilliantw.net', label: '羽森服' },
    { host: 's4.brilliantw.net', label: '恐龍服' }
];

const PING_INTERVAL_MS = 60_000;
let busy = false;
let statuses = [];

const shardOf = (host) => SHARDS.find((shard) => shard.host === host) ?? null;
const REMIND_KEY = 'eventReminders';
let restarts = {};
let events = [];

export const upcomingEvent = () => events.find((ev) => ev.active || ev.start > Date.now()) ?? null;

function taipeiNow() {
    const [hh, mm] = new Intl.DateTimeFormat('en-GB', { timeZone: 'Asia/Taipei', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(new Date()).split(':').map(Number);
    return hh * 60 + mm;
}

export function restartOf(label, now = taipeiNow(), schedule = restarts) {
    let next = null;
    let recent = false;
    for (const time of schedule[label] || []) {
        const [hh, mm] = time.split(':').map(Number);
        const at = hh * 60 + mm;
        const until = (at - now + 1440) % 1440;
        if (!next || until < next.mins) next = { time, mins: until };
        if ((now - at + 1440) % 1440 <= 3) recent = true;
    }
    return { next, recent };
}

async function showLaunchError(instance, error) {
    const message = String(error?.message ?? error);
    const value = await dialog({
        title: '無法啟動遊戲',
        content: h('p', { style: 'white-space:pre-wrap' }, message),
        buttons: [{ label: '回報給管理員', value: 'report' }, { label: '關閉', value: null }]
    });
    if (value === 'report') reportDialog('launch', instance, { message });
}

const reminders = () => {
    try {
        return JSON.parse(localStorage.getItem(REMIND_KEY) || '[]');
    } catch {
        return [];
    }
};
const setReminders = (ids) => {
    try {
        localStorage.setItem(REMIND_KEY, JSON.stringify(ids.slice(-20)));
    } catch {}
};
const EVENT_SHARDS = [...SHARDS, { host: 's0.brilliantw.net', label: '時隙奉廳' }];
const shardIn = (text) => EVENT_SHARDS.find((shard) => String(text).includes(shard.label.slice(0, 2)));

function eventTime(e) {
    if (e.active) return '進行中';
    const mins = Math.round((e.start - Date.now()) / 60000);
    const rel = mins < 60 ? `${Math.max(mins, 0)} 分鐘後` : mins < 1440 ? `${Math.round(mins / 60)} 小時後` : `${Math.round(mins / 1440)} 天後`;
    return `${new Date(e.start).toLocaleString('zh-TW', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false })}（${rel}）`;
}

async function showEvent(e) {
    const value = await dialog({
        title: e.name,
        content: h('div', { class: 'event-detail' },
            e.image ? h('img', { src: e.image, alt: '' }) : null,
            h('p', { class: 'hint' }, `${eventTime(e)}${e.location ? `｜${e.location}` : ''}`),
            e.description ? h('p', { style: 'white-space:pre-wrap' }, e.description) : null),
        buttons: [{ label: '在 Discord 查看', value: 'web' }, { label: '關閉', value: null }]
    });
    if (value === 'web') openUrl(e.url);
}

function renderEvent() {
    const box = $('#event-card');
    const e = events.find((ev) => ev.active || ev.start > Date.now());
    box.hidden = !e;
    if (!e) return;
    const on = reminders().includes(e.id);
    const shard = shardIn(`${e.location} ${e.name}`);
    clear(box).append(
        h('div', { class: 'event-head' }, icon('gift'), e.active ? '活動進行中' : '即將舉行的活動'),
        h('button', { type: 'button', class: 'event-name', title: '查看活動詳情', onclick: () => showEvent(e) }, e.name),
        h('div', { class: 'event-time' }, `${eventTime(e)}${e.location ? `｜${e.location}` : ''}`),
        h('div', { class: 'event-actions' },
            e.active ? null : h('button', {
                type: 'button',
                class: `mc ${on ? '' : 'primary'}`,
                onclick: () => {
                    setReminders(on ? reminders().filter((id) => id !== e.id) : [...reminders(), e.id]);
                    toast(on ? '已取消提醒' : '活動開始前 10 分鐘會通知你（啟動器要開著或縮在系統匣）');
                    renderEvent();
                }
            }, on ? '已設定提醒' : '提醒我'),
            shard ? h('button', { type: 'button', class: 'mc', onclick: () => joinServer(shard.host, shard.label) }, `前往${shard.label}`) : null));
}

async function refreshEvents() {
    const data = await api.site_json({ path: '/api/launcher/events' }).catch(() => null);
    if (Array.isArray(data?.events)) events = data.events;
    renderEvent();
    emit('events');
}

function remindEvents() {
    const ids = reminders();
    for (const e of events) {
        const left = e.start - Date.now();
        if (ids.includes(e.id) && left <= 10 * 60000 && left > -30 * 60000) {
            notifyOnce(`event:${e.id}`, `活動即將開始：${e.name}`, e.location ? `地點：${e.location}` : '打開輝煌啟動器看看詳情');
        }
    }
}

function renderAccountChip() {
    const account = selectedAccount();
    $('#chip-avatar').dataset.avatar = account?.id ?? '';
    $('#chip-avatar').src = avatarUrl(account);
    $('#chip-name').textContent = account ? account.name : '登入帳號';
}

function renderPlayBar() {
    const instance = selectedInstance();
    const account = accountFor(instance);
    const running = instance && isRunning(instance.id);
    const target = instance?.general ? null : shardOf(store.settings?.quickJoin);
    const button = $('#btn-play');

    clear($('#home-thumb')).append(...(instance ? [instanceVisual(instance)] : []));
    clear($('#home-instance')).append(...[h('span', { class: 'pick-title' }, instance ? instance.name : '尚未建立實例'), instance && account ? h('img', { class: 'name-face', src: avatarUrl(account), alt: '', title: account.name }) : null].filter(Boolean));
    $('#instance-pick').title = instance ? `${instance.name}｜Minecraft ${instance.mcVersion}${instance.loader === 'fabric' ? `・Fabric ${instance.loaderVersion || ''}` : '・原版'}${account ? `｜${account.name}` : ''}（點一下切換實例）` : '切換遊戲實例';
    clear($('#home-meta')).append(...(!instance
        ? ['點右邊按鈕即可建立']
        : [
            h('span', { class: 'meta-fixed' }, `${instance.mcVersion}・${instance.loader === 'fabric' ? 'Fabric' : '原版'}`)
        ]));
    renderUpgrade(instance);
    document.body.classList.toggle('launching', busy);

    const action = running ? 'stop' : busy ? 'busy' : !instance ? 'create' : !account ? 'login' : 'play';
    const where = target ? `直連 ${target.label}` : '進入主畫面';
    const [label, sub] = {
        stop: ['結束遊戲', ''],
        busy: ['準備中⋯', ''],
        create: ['建立實例', '一鍵建立輝煌實例'],
        login: ['登入帳號', '尚未登入，無法開始遊戲'],
        play: [store.settings?.playText?.trim() || '開始遊戲', ready.has(instance?.id) ? `已就緒・${where}` : where]
    }[action];
    clear(button).append(label, sub ? h('small', {}, sub) : null);
    button.dataset.action = action;
    button.classList.toggle('stop', action === 'stop');
    button.classList.toggle('busy', action === 'busy');
    button.disabled = action === 'busy';
    renderAccountChip();
    renderSetup(account, instance);
}

let creating = false;

const dismissed = new Set();

function renderUpgrade(instance) {
    const box = $('#upgrade-banner');
    const target = upgradeTarget(instance);
    box.hidden = !target || dismissed.has(`${instance?.id}:${target}`) || busy;
    if (box.hidden) return;
    clear(box).append(
        h('div', { class: 'grow' },
            h('b', {}, `伺服器已更新到 Minecraft ${target}`),
            h('small', {}, `「${instance.name}」還在 ${instance.mcVersion}，一鍵升級並自動更新模組。`)),
        h('button', { type: 'button', class: 'mc primary', onclick: () => upgradeFlow(instance, target) }, '一鍵升級'),
        h('button', { type: 'button', class: 'mc icon-btn', title: '稍後', 'aria-label': '稍後', onclick: () => { dismissed.add(`${instance.id}:${target}`); renderUpgrade(instance); } }, icon('close')));
}

function renderSetup(account, instance) {
    const box = $('#setup');
    box.hidden = !!(account && instance);
    if (box.hidden) return;
    const step = (n, title, text, ...actions) =>
        clear(box).append(
            h('div', { class: 'setup-step' }, `第 ${n} 步（共 2 步）`),
            h('div', { class: 'setup-title' }, title),
            h('div', { class: 'setup-text' }, text),
            h('div', { class: 'setup-actions' }, ...actions)
        );
    if (!account) {
        step(1, '登入帳號', '用微軟帳號登入，就能直接連上輝煌的各個分流。',
            h('button', { type: 'button', class: 'mc primary', onclick: loginMicrosoft }, '微軟正版登入'),
            h('button', { type: 'button', class: 'mc', onclick: addOffline }, '離線帳號'));
    } else {
        step(2, '建立遊戲實例', '一鍵安裝輝煌推薦的 Minecraft 版本、Fabric 與 Brilliant-Server-Mod。想玩合作伺服器或單人遊戲，也可以建立一般實例。',
            h('button', {
                type: 'button',
                class: 'mc primary',
                disabled: creating,
                onclick: async () => {
                    creating = true;
                    renderPlayBar();
                    await quickCreate().catch(fail);
                    creating = false;
                    renderPlayBar();
                }
            }, creating ? '建立中⋯' : '一鍵建立輝煌實例'),
            h('button', { type: 'button', class: 'mc', disabled: creating, onclick: () => createGeneralInstance() }, '建立一般實例'));
    }
}

function setProgress({ label, doneFiles, totalFiles, doneBytes, totalBytes }) {
    $('#progress').hidden = false;
    const ratio = totalBytes > 0 ? doneBytes / totalBytes : totalFiles > 0 ? doneFiles / totalFiles : 0;
    $('#progress-fill').style.width = `${Math.min(100, ratio * 100).toFixed(1)}%`;
    $('#progress-label').textContent = label || '準備中⋯⋯';
    $('#progress-count').textContent = totalFiles
        ? `${doneFiles} / ${totalFiles}　${fmtBytes(doneBytes)} / ${fmtBytes(totalBytes)}`
        : '';
}

const STAGE_TEXT = {
    version: '解析版本',
    libraries: '下載函式庫',
    assets: '下載資源檔',
    java: '下載 Java 執行環境',
    mods: '同步模組',
    respack: '下載伺服器資源包',
    backup: '備份存檔',
    hook: '執行指令',
    launch: '啟動遊戲'
};

async function splash(title, sub) {
    const box = $('#launch-splash');
    $('#splash-title').textContent = title;
    $('#splash-sub').textContent = sub;
    box.hidden = false;
    await new Promise((done) => setTimeout(done, 1800));
    box.classList.add('out');
    setTimeout(() => {
        box.hidden = true;
        box.classList.remove('out');
    }, store.settings?.afterLaunch === 'keep' ? 400 : 0);
}

async function play(relogged = false, join = null) {
    const instance = selectedInstance();
    if (!instance) return;
    if (busy && relogged !== true) return toast('正在準備遊戲，請等這次完成', 'warn');

    if (isRunning(instance.id)) {
        if (!(await confirm(`要強制結束「${instance.name}」嗎？\n強制結束不會存檔，單人世界最近的進度可能遺失；建議先在遊戲裡按「儲存並離開」或關閉遊戲視窗。`, '強制結束'))) return;
        await api.stop_game({ id: instance.id }).then(() => toast(`已強制結束「${instance.name}」`), fail);
        return;
    }

    const goingTo = !join && !instance.general ? shardOf(store.settings?.quickJoin) : SHARDS.find((s) => s.host === join?.host);
    const restart = goingTo ? restartOf(goingTo.label) : {};
    if (restart.recent && !(await api.ping_server({ host: goingTo.host }).catch(() => null))) {
        await dialog({
            title: `${goingTo.label}正在每日重啟`,
            content: h('p', {}, `${goingTo.label}剛進行每日重啟，通常 1～3 分鐘內就會恢復，請稍後再開始遊戲。`),
            buttons: [{ label: '知道了', value: null }]
        });
        return;
    }
    const soon = restart.next;
    if (soon && soon.mins <= 5 && !(await confirm(`${goingTo.label}將在 ${soon.time} 每日重啟（${soon.mins ? `約 ${soon.mins} 分鐘後` : '就是現在'}），現在進去很快就會斷線。
仍要開始遊戲嗎？`, '仍要開始'))) return;

    busy = true;
    clearRecentLog(instance.id);
    renderPlayBar();
    setProgress({ label: '準備中⋯⋯', doneFiles: 0, totalFiles: 0, doneBytes: 0, totalBytes: 0 });
    try {
        const notes = await api.launch_game({ id: instance.id, join: join?.host ?? (instance.general ? '' : null) });
        for (const note of notes) toast(note, 'warn');
        const target = instance.general ? null : shardOf(store.settings?.quickJoin);
        toast(join ? `遊戲已啟動，將直接連線 ${join.name}` : target ? `遊戲已啟動，將直接連線 ${target.label}` : '遊戲已啟動');
        if (store.settings?.launchSplash) await splash(instance.name, join ? `直連 ${join.name}` : target ? `直連 ${target.label}` : '進入主畫面');
        await loadInstances();
        if (store.settings?.openLogOnLaunch) location.hash = '#log';
        const after = store.settings?.afterLaunch;
        if (after === 'hide') await api.hide_window();
        else if (after === 'exit') await api.exit_launcher();
    } catch (error) {
        if (relogged !== true && /重新登入/.test(String(error?.message ?? error))) {
            busy = false;
            $('#progress').hidden = true;
            renderPlayBar();
            toast('登入已過期，請在跳出的視窗重新登入，完成後會自動開始遊戲');
            if (await loginMicrosoft()) return play(true, join);
            return;
        }
        showLaunchError(instance, error);
    } finally {
        busy = false;
        $('#progress').hidden = true;
        renderPlayBar();
    }
}

export const isPreparing = () => busy;

export function joinServer(host, name) {
    if (busy) return toast('正在準備遊戲，請等這次完成後再啟動', 'warn');
    location.hash = '#home';
    play(false, { host, name });
}

export async function repairInstance(instance) {
    if (busy || !instance) return;
    if (isRunning(instance.id)) return toast('請先關閉遊戲再修復', 'warn');
    busy = true;
    renderPlayBar();
    setProgress({ label: '驗證檔案中⋯⋯', doneFiles: 0, totalFiles: 0, doneBytes: 0, totalBytes: 0 });
    toast(`正在逐一驗證「${instance.name}」的遊戲檔案，損壞的會重新下載⋯`);
    try {
        await api.repair_instance({ id: instance.id });
        toast('檔案驗證完成，損壞或缺少的檔案都已修復');
    } catch (error) {
        fail(error);
    } finally {
        busy = false;
        $('#progress').hidden = true;
        renderPlayBar();
    }
}

async function runFix(instance, fix) {
    const id = instance.id;
    if (fix.kind === 'disable') return api.set_mod_enabled({ id, file: fix.target, enabled: false });
    if (fix.kind === 'install') return api.content_install({ id, kind: 'mod', project: fix.target });
    if (fix.kind === 'memory') return api.apply_perf({ id, options: {}, memoryMb: fix.target });
    if (fix.kind === 'java') return api.update_instance({ updated: { ...instance, javaMajor: fix.target } });
    if (fix.kind === 'loader') return api.update_instance({ updated: { ...instance, loaderVersion: '' } });
    if (fix.kind === 'dgpu') return saveSettings({ preferDgpu: true });
    if (fix.kind === 'repair') return repairInstance(instance);
    if (fix.kind === 'update') {
        const entry = (await api.content_list({ id, kind: 'mod', mc: instance.mcVersion })).find((e) => e.file === fix.target);
        if (entry?.update) return api.content_update({ id, kind: 'mod', file: entry.file, update: entry.update });
        return api.set_mod_enabled({ id, file: fix.target, enabled: false });
    }
}

async function showCrash(id, code, seconds) {
    const instance = store.instances.find((i) => i.id === id) ?? selectedInstance();
    const lines = recentLog(instance.id);
    const [ctx, info] = await Promise.all([
        api.crash_context({ id: instance.id, since: Math.floor(Date.now() / 1000) - seconds - 5, lines: lines.slice(-400) }).catch(() => ({ report: '', native: '', mods: [] })),
        api.system_info().catch(() => ({}))
    ]);
    const { findings, exception, fixes } = plan({
        lines, ...ctx,
        memoryMb: instance.memoryMb || store.settings?.memoryMb || 0,
        totalMb: info.memoryMb, recommendedMb: info.recommendedMb,
        preferDgpu: store.settings?.preferDgpu
    });
    const boxes = fixes.map((fix) => ({ fix, input: h('input', { type: 'checkbox', checked: true }) }));
    const content = h('div', { class: 'crash' },
        findings.length
            ? h('div', {}, ...findings.map((f) => h('div', { class: 'crash-item' }, h('b', {}, f.title), h('p', {}, f.detail))))
            : h('p', {}, '沒有找到常見的原因。請按「回報給管理員」或「複製日誌」，貼到輝煌 Discord 的問題回報頻道，管理員會協助查看。'),
        exception ? h('pre', { class: 'crash-exception' }, exception) : '',
        boxes.length ? h('div', { class: 'crash-fixes' },
            h('b', {}, '一鍵修復會做這些事：'),
            ...boxes.map(({ fix, input }) => h('label', {}, input, fix.label))) : '');
    const value = await dialog({
        title: `「${instance.name}」異常結束（代碼 ${code}）`,
        content,
        buttons: [
            ...(boxes.length ? [{ label: '一鍵修復', value: 'fix' }] : [{ label: '修復遊戲檔案', value: 'repair' }]),
            { label: '回報給管理員', value: 'report' },
            { label: '複製日誌', value: 'copy' },
            { label: '查看日誌', value: 'log' },
            { label: '關閉', value: null }
        ]
    });
    if (value === 'copy') {
        await navigator.clipboard.writeText([ctx.report, ...lines].filter(Boolean).join('\n')).then(() => toast('已複製日誌'), fail);
    } else if (value === 'log') {
        location.hash = '#log';
    } else if (value === 'repair') {
        repairInstance(instance);
    } else if (value === 'report') {
        reportDialog('crash', instance, { code, findings: findings.map((f) => f.title), exception, since: Math.floor(Date.now() / 1000) - seconds - 5 });
    } else if (value === 'fix') {
        const chosen = boxes.filter((b) => b.input.checked).map((b) => b.fix);
        let done = 0;
        for (const fix of chosen) {
            await runFix(instance, fix).then(() => { done += 1; }, fail);
        }
        await loadInstances().catch(() => {});
        if (!done) return;
        if (await confirm(`已完成 ${done} 項修復。要現在重新開始遊戲嗎？`, '開始遊戲')) {
            if (selectedInstance()?.id !== instance.id) {
                await api.select_instance({ id: instance.id });
                store.settings.selectedInstance = instance.id;
                emit('settings');
            }
            play();
        }
    }
}

function shardTile(shard, status) {
    const selected = store.settings?.quickJoin === shard.host;
    const state = status === undefined ? 'pending' : status ? 'up' : 'down';
    const restart = restartOf(shard.label);
    const busiest = Math.max(1, ...statuses.map((item) => item?.online ?? 0));
    const ratio = status?.online != null ? Math.min(1, status.online / busiest) : 0;
    return h(
        'button',
        {
            type: 'button',
            class: `shard glass ${state}`,
            'aria-pressed': selected ? 'true' : 'false',
            title: store.settings?.streamerMode ? shard.label : status ? `${shard.host}\n${status.motd}` : shard.host,
            onclick: async () => {
                await saveSettings({ quickJoin: selected ? '' : shard.host }).catch(fail);
                renderShards();
                renderPlayBar();
            }
        },
        h('span', { class: 'check' }, icon('check'), '直連'),
        h('div', { class: 'name' }, shard.label),
        h(
            'div',
            { class: 'meta' },
            h('span', { class: 'dot' }),
            state === 'pending'
                ? '查詢中⋯'
                : status
                    ? status.online != null
                        ? h('span', {}, h('b', {}, String(status.online)), ` 人　${status.latencyMs}ms`)
                        : h('span', {}, `可連線　${status.latencyMs}ms`)
                    : restart.recent ? '每日重啟中，稍後恢復' : '目前連不上'
        ),
        restart.next && restart.next.mins <= 30 ? h('div', { class: 'restart-soon' }, `${restart.next.time} 每日重啟`) : null,
        h('div', { class: 'bar' }, h('i', { style: `width:${(ratio * 100).toFixed(1)}%` }))
    );
}

function renderShards() {
    const hidden = store.settings?.hiddenShards ?? [];
    clear($('#shards')).append(...SHARDS.map((shard, i) => (hidden.includes(shard.host) ? null : shardTile(shard, statuses[i]))).filter(Boolean));
    const target = shardOf(store.settings?.quickJoin);
    clear($('#shard-hint')).append(...(selectedInstance()?.general
        ? ['這是一般實例，開始遊戲後會停在主畫面；分流選擇只套用在輝煌實例。']
        : target
        ? ['開始遊戲後直接進入 ', h('b', {}, target.label), h('button', {
            type: 'button',
            onclick: async () => {
                await saveSettings({ quickJoin: '' }).catch(fail);
                renderShards();
                renderPlayBar();
            }
        }, '改為進入主畫面')]
        : ['點選分流，開始遊戲後就會直接進入；不選則停在主畫面。']));
    const counted = statuses.filter((status) => status?.online != null);
    const total = counted.reduce((sum, status) => sum + status.online, 0);
    clear($('#shard-total')).append(counted.length ? h('span', {}, '四個分流共 ', h('b', {}, String(total)), ' 人在線') : '');
}

async function refreshShards() {
    const [online, pings] = await Promise.all([
        api.site_json({ path: '/api/online' }).catch(() => null),
        Promise.all(SHARDS.map((shard) => api.ping_server({ host: shard.host }).catch(() => null)))
    ]);
    statuses = pings.map((ping, i) => {
        if (!ping) return null;
        const count = online?.servers?.find((server) => server.name === SHARDS[i].label)?.count;
        return { ...ping, online: Number.isFinite(count) ? count : null };
    });
    renderShards();
}

function toggleAccountMenu(force) {
    const menu = $('#account-menu');
    const open = force ?? menu.hidden;
    menu.hidden = !open;
    $('#account-more').setAttribute('aria-expanded', String(open));
    if (!open) return;
    const current = selectedAccount();
    clear(menu).append(
        ...store.accounts.map((account) => h('button', {
            type: 'button',
            role: 'menuitem',
            'aria-current': current?.id === account.id ? 'true' : 'false',
            onclick: async () => {
                toggleAccountMenu(false);
                if (current?.id === account.id) return;
                await api.select_account({ id: account.id }).catch(fail);
                store.settings.selectedAccount = account.id;
                emit('accounts');
                emit('settings');
                toast(`已切換為 ${account.name}`);
            }
        }, h('img', { src: avatarUrl(account), alt: '' }), account.name, h('span', { class: 'sub' }, account.kind === 'msa' ? '正版' : '離線'))),
        store.accounts.length ? h('hr') : null,
        h('button', { type: 'button', role: 'menuitem', onclick: () => { toggleAccountMenu(false); location.hash = '#accounts'; } },
            icon('user'), store.accounts.length ? '管理帳號' : '登入帳號')
    );
}

function toggleInstanceMenu(force) {
    const menu = $('#instance-menu');
    const open = force ?? menu.hidden;
    menu.hidden = !open;
    if (!open) return;
    const current = selectedInstance();
    clear(menu).append(
        ...store.instances.map((instance) =>
            h(
                'button',
                {
                    type: 'button',
                    'aria-current': current?.id === instance.id ? 'true' : 'false',
                    onclick: async () => {
                        toggleInstanceMenu(false);
                        await api.select_instance({ id: instance.id }).catch(fail);
                        store.settings.selectedInstance = instance.id;
                        emit('instances');
                        emit('settings');
                    }
                },
                h('span', { class: 'inst-thumb' }, instanceVisual(instance)),
                h('span', { class: 'menu-name' }, instance.name),
                h('span', { class: 'sub' }, `${instance.mcVersion}${instance.loader === 'fabric' ? '・Fabric' : ''}`)
            )
        ),
        store.instances.length ? h('hr') : '',
        h('button', { type: 'button', onclick: () => { toggleInstanceMenu(false); location.hash = '#instances'; } }, '管理實例⋯')
    );
}

const FOLD_KEY = 'brilliant.home.fold';

function readFold() {
    try {
        return JSON.parse(localStorage.getItem(FOLD_KEY)) || {};
    } catch {
        return {};
    }
}

function setFold(patch) {
    const fold = { ...readFold(), ...patch };
    try {
        localStorage.setItem(FOLD_KEY, JSON.stringify(fold));
    } catch {}
    applyFold(fold);
}

function applyFold(fold = readFold()) {
    const body = document.body;
    body.classList.toggle('fold-news', !!fold.news);
    body.classList.toggle('fold-dock', !!fold.dock);
    body.classList.toggle('scenic', !!fold.scenic);
    $('#fold-news').setAttribute('aria-expanded', String(!fold.news));
    $('#fold-dock').setAttribute('aria-expanded', String(!fold.dock));
    $('#scenic-toggle').setAttribute('aria-pressed', String(!!fold.scenic));
    $('#scenic-toggle').replaceChildren(icon(fold.scenic ? 'eye-closed' : 'eye'));
}

function initFold() {
    const newsFold = h('button', { type: 'button', class: 'fold', id: 'fold-news', title: '收合公告', onclick: () => setFold({ news: !readFold().news }) }, icon('chevron-down'));
    $('.news').prepend(newsFold);
    $('#fold-dock').addEventListener('click', () => setFold({ dock: !readFold().dock }));
    $('#scenic-toggle').addEventListener('click', () => setFold({ scenic: !readFold().scenic }));
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && document.body.classList.contains('scenic') && !document.querySelector('dialog[open]')) setFold({ scenic: false });
    });
    applyFold();
}

export function initHome() {
    initFold();
    $('#btn-play').addEventListener('click', () => {
        const action = $('#btn-play').dataset.action;
        if (action === 'create') return quickCreate().catch(fail);
        if (action === 'login') return loginMicrosoft().catch(fail);
        play();
    });
    on('launch', () => (busy ? toast('正在準備遊戲，請等這次完成後再啟動', 'warn') : play()));
    $('#instance-pick').classList.add('glass');
    $('#instance-pick').addEventListener('click', (e) => {
        e.stopPropagation();
        toggleInstanceMenu();
    });
    $('#instance-menu').classList.add('glass');
    document.addEventListener('click', (e) => {
        if (!e.target.closest('#instance-menu')) toggleInstanceMenu(false);
        if (!e.target.closest('#account-menu, #account-more')) toggleAccountMenu(false);
    });
    $('#account-more').addEventListener('click', () => toggleAccountMenu());
    on('instances', () => {
        renderPlayBar();
        renderShards();
    });
    on('accounts', renderPlayBar);
    on('ready', renderPlayBar);
    versionData().then(renderPlayBar, () => {});
    on('settings', () => {
        renderPlayBar();
        renderShards();
    });

    listen('launcher://progress', (p) => {
        if (!busy) return;
        setProgress({ ...p, label: `${STAGE_TEXT[p.stage] ?? p.stage}：${p.label}` });
    });
    listen('launcher://tray-play', () => emit('launch'));
    listen('launcher://cli-launch', async ({ name, error }) => {
        await loadInstances().catch(() => {});
        if (error) toast(`捷徑啟動「${name}」失敗：${error}`, 'error');
        else toast(`已從捷徑啟動「${name}」`);
    });
    listen('launcher://adopted', async (id) => {
        await loadInstances().catch(() => {});
        const name = store.instances.find((i) => i.id === id)?.name;
        if (name) toast(`已重新連上執行中的「${name}」，日誌會從遊戲的 latest.log 接續顯示`);
    });
    listen('launcher://exit', async ({ instance, code, seconds }) => {
        await api.show_window().catch(() => {});
        await loadInstances().catch(() => {});
        if (code === 0) {
            toast(`遊戲已結束（遊玩 ${Math.round(seconds / 60)} 分鐘）`);
            perfAfterExit(instance, seconds);
        } else showCrash(instance, code, seconds);
    });

    renderPlayBar();
    renderShards();
    refreshShards().catch(() => {});
    setInterval(() => { if (!document.hidden) refreshShards().catch(() => {}); }, PING_INTERVAL_MS);
    document.addEventListener('visibilitychange', () => { if (!document.hidden) refreshShards().catch(() => {}); });
    on('network', () => {
        if (store.online === false) return;
        refreshShards().catch(() => {});
        refreshEvents();
        loadAccounts().catch(() => {});
    });
    initNews();
    api.site_json({ path: '/api/launcher/restarts' }).then((data) => { restarts = data?.schedule || {}; renderShards(); }, () => {});
    refreshEvents();
    setInterval(refreshEvents, 10 * 60 * 1000);
    setInterval(() => {
        remindEvents();
        renderEvent();
    }, 60 * 1000);
}
