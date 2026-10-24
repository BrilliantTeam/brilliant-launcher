import { api, fileSrc, watchFolders } from '../api.js';
import { $, clear, confirm, dialog, fail, guard, h, toast, RECYCLE_NOTE } from '../ui.js';
import { icon } from '../icons.js';
import { on, saveSettings, selectedAccount, store } from '../store.js';
import { shareScreenshot } from './instances.js';
import { byMonth, caption, fmtDate, memories } from '../memories.js';

const DAY = 86400;
const SNAP_KEY = 'recapSnapshots';
let shots = [];
let filter = '';

const hours = (s) => (s >= 3600 ? `${(s / 3600).toFixed(1)} 小時` : `${Math.round(s / 60)} 分鐘`);
const dayKey = (t) => new Date(t * 1000).toLocaleDateString('zh-TW');

export function weekRecap(sessions, shotList, now) {
    const from = now - 7 * DAY;
    const week = sessions.filter((s) => s.start >= from);
    const prev = sessions.filter((s) => s.start >= from - 7 * DAY && s.start < from);
    const sum = (list) => list.reduce((a, s) => a + Math.max(0, s.seconds), 0);
    const byInstance = {};
    for (const s of week) byInstance[s.name] = (byInstance[s.name] ?? 0) + s.seconds;
    const top = Object.entries(byInstance).sort((a, b) => b[1] - a[1])[0] ?? null;
    const longest = week.reduce((a, s) => (s.seconds > (a?.seconds ?? 0) ? s : a), null);
    return {
        seconds: sum(week),
        prevSeconds: sum(prev),
        sessions: week.length,
        days: new Set(week.map((s) => dayKey(s.start))).size,
        top: top ? { name: top[0], seconds: top[1] } : null,
        longest,
        shots: shotList.filter((s) => s.time >= from)
    };
}

function readSnaps() {
    try {
        return JSON.parse(localStorage.getItem(SNAP_KEY) || '{}');
    } catch {
        return {};
    }
}

async function brilliantDelta(now) {
    const account = selectedAccount();
    if (account?.kind !== 'msa') return [];
    const uuid = account.uuid.replace(/-/g, '');
    const data = await api.site_json({ path: `/mcstats/api/player?uuid=${uuid}` }).catch(() => null);
    if (!data?.servers?.length) return [];
    const totals = Object.fromEntries(Object.keys(data.metrics).map((k) => [k, data.servers.reduce((a, s) => a + (s.stats[k]?.value || 0), 0)]));
    const all = readSnaps();
    const mine = (all[uuid] ?? []).filter((s) => s.t >= now - 30 * DAY);
    if (!mine.some((s) => dayKey(s.t) === dayKey(now))) mine.push({ t: now, totals });
    all[uuid] = mine;
    try {
        localStorage.setItem(SNAP_KEY, JSON.stringify(all));
    } catch {}
    const base = mine.filter((s) => s.t <= now - 6 * DAY).at(-1) ?? null;
    if (!base) return [];
    return Object.keys(totals)
        .map((k) => ({ key: k, label: data.metrics[k].label, format: data.metrics[k].format, delta: totals[k] - (base.totals[k] ?? 0) }))
        .filter((d) => d.delta > 0 && d.format !== 'duration')
        .sort((a, b) => b.delta - a.delta)
        .slice(0, 3);
}

function recapCard(r, delta) {
    const diff = r.seconds - r.prevSeconds;
    const stat = (value, label) => h('div', { class: 'recap-stat' }, h('b', {}, value), h('span', {}, label));
    const cover = r.shots[0];
    return h('div', { class: 'recap glass', style: cover ? `--cover:url("${fileSrc(cover.path)}")` : '' },
        h('div', { class: 'recap-head' },
            h('div', {}, h('div', { class: 'recap-title' }, '本週回顧'), h('div', { class: 'meta' }, `${dayKey(Date.now() / 1000 - 6 * DAY)} ～ ${dayKey(Date.now() / 1000)}`)),
            h('button', { type: 'button', class: 'mc', onclick: guard(() => saveRecapImage(r, delta)) }, icon('download'), '存成圖片')),
        r.sessions
            ? h('div', { class: 'recap-stats' },
                stat(hours(r.seconds), r.prevSeconds ? `遊玩時間（比上週${diff >= 0 ? '多' : '少'} ${hours(Math.abs(diff))}）` : '遊玩時間'),
                stat(`${r.days} 天`, `上線天數・共 ${r.sessions} 次`),
                stat(r.top?.name ?? '—', `最常玩（${hours(r.top?.seconds ?? 0)}）`),
                stat(`${r.shots.length} 張`, '新截圖'),
                ...delta.map((d) => stat(`+${d.delta.toLocaleString('zh-TW')}`, `輝煌・${d.label}`)))
            : h('p', { class: 'hint' }, '這週還沒有遊玩紀錄。從這個版本開始，啟動器會記錄每次遊玩，下週就能看到你的回顧。'));
}

function loadImage(src) {
    return new Promise((resolve) => {
        const img = new Image();
        img.crossOrigin = 'anonymous';
        img.onload = () => resolve(img);
        img.onerror = () => resolve(null);
        img.src = src;
    });
}

async function drawRecap(r, delta, cover) {
    const canvas = document.createElement('canvas');
    canvas.width = 1280;
    canvas.height = 720;
    const g = canvas.getContext('2d');
    g.fillStyle = '#1b1d22';
    g.fillRect(0, 0, 1280, 720);
    if (cover) {
        const scale = Math.max(1280 / cover.width, 720 / cover.height);
        g.drawImage(cover, (1280 - cover.width * scale) / 2, (720 - cover.height * scale) / 2, cover.width * scale, cover.height * scale);
        g.fillStyle = 'rgba(0,0,0,0.55)';
        g.fillRect(0, 0, 1280, 720);
    }
    const accent = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#f5b041';
    const font = getComputedStyle(document.body).fontFamily;
    g.fillStyle = accent;
    g.fillRect(64, 64, 8, 92);
    g.fillStyle = '#ffffff';
    g.font = `bold 56px ${font}`;
    g.fillText('我的輝煌一週', 92, 118);
    g.font = `26px ${font}`;
    g.fillStyle = '#d0d0d0';
    g.fillText(`${selectedAccount()?.name ?? ''}・${dayKey(Date.now() / 1000 - 6 * DAY)} ～ ${dayKey(Date.now() / 1000)}`, 92, 156);
    const items = [
        [hours(r.seconds), '遊玩時間'],
        [`${r.days} 天`, '上線天數'],
        [r.top?.name ?? '—', '最常玩'],
        [`${r.shots.length} 張`, '新截圖'],
        ...delta.map((d) => [`+${d.delta.toLocaleString('zh-TW')}`, d.label])
    ].slice(0, 6);
    items.forEach(([value, label], i) => {
        const x = 92 + (i % 3) * 380;
        const y = 300 + Math.floor(i / 3) * 170;
        g.fillStyle = '#ffffff';
        g.font = `bold 52px ${font}`;
        g.fillText(String(value).slice(0, 12), x, y);
        g.fillStyle = accent;
        g.font = `26px ${font}`;
        g.fillText(label, x, y + 44);
    });
    g.fillStyle = '#bbbbbb';
    g.font = `22px ${font}`;
    g.fillText('輝煌啟動器・brilliantw.net', 92, 664);
    return canvas.toDataURL('image/png');
}

async function saveRecapImage(r, delta) {
    const cover = r.shots[0] ? await loadImage(fileSrc(r.shots[0].path)) : null;
    let data;
    try {
        data = await drawRecap(r, delta, cover);
    } catch {
        data = await drawRecap(r, delta, null);
    }
    const path = await api.save_png({ name: 'brilliant-week.png', data });
    toast(`已存到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
    api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
}

function memoryCard(memory) {
    return h('button', { type: 'button', class: 'memory', style: `background-image:url("${fileSrc(memory.cover.thumb || memory.cover.path)}")`, onclick: () => playMemory(memory) },
        h('span', { class: 'memory-count' }, `${memory.shots.length} 張`),
        h('span', { class: 'memory-text' }, h('b', {}, memory.title), h('small', {}, memory.subtitle)));
}

async function playMemory(memory) {
    let index = 0;
    let timer = null;
    const many = memory.shots.length > 1;
    const img = h('img', { alt: '' });
    const info = h('div', { class: 'story-info' });
    const dots = h('div', { class: 'story-dots' }, ...memory.shots.map(() => h('span')));
    const playBtn = h('button', { type: 'button', class: 'mc', onclick: () => (timer ? stop() : start()) });
    const show = (i) => {
        index = (i + memory.shots.length) % memory.shots.length;
        const shot = memory.shots[index];
        img.classList.remove('kb');
        img.src = fileSrc(shot.path);
        void img.offsetWidth;
        img.classList.add('kb');
        clear(info).append(h('b', {}, fmtDate(shot.time)), h('span', {}, [shot.instanceName, caption(shot)].filter(Boolean).join('｜')));
        [...dots.children].forEach((d, n) => d.classList.toggle('on', n === index));
    };
    const start = () => { timer = setInterval(() => show(index + 1), 4000); playBtn.replaceChildren(icon('close'), '暫停'); };
    const stop = () => { clearInterval(timer); timer = null; playBtn.replaceChildren(icon('play'), '自動播放'); };
    const step = (n) => { stop(); show(index + n); };
    const key = (e) => {
        if (e.key === 'ArrowRight') step(1);
        if (e.key === 'ArrowLeft') step(-1);
    };
    show(0);
    if (many) start(); else stop();
    addEventListener('keydown', key);
    await dialog({
        title: memory.title,
        wide: true,
        content: h('div', { class: 'story' },
            h('div', { class: 'story-stage' }, img,
                many ? h('button', { type: 'button', class: 'story-nav prev', 'aria-label': '上一張', onclick: () => step(-1) }, '‹') : '',
                many ? h('button', { type: 'button', class: 'story-nav next', 'aria-label': '下一張', onclick: () => step(1) }, '›') : ''),
            info,
            many ? h('div', { class: 'story-bar' }, dots, playBtn) : '')
    });
    clearInterval(timer);
    removeEventListener('keydown', key);
}

async function viewShot(shot) {
    const instance = store.instances.find((i) => i.id === shot.instance);
    const value = await dialog({
        title: `${shot.instanceName}｜${new Date(shot.time * 1000).toLocaleString('zh-TW', { hour12: false })}`,
        wide: true,
        content: h('div', {}, h('img', { class: 'shot-view', src: fileSrc(shot.path), alt: shot.name }), caption(shot) ? h('p', { class: 'hint shot-caption' }, caption(shot)) : ''),
        buttons: [
            { label: '投稿到玩家足跡', value: 'share' },
            { label: '設為啟動器背景', value: 'bg' },
            { label: '開啟', value: 'open' },
            { label: '刪除', value: 'delete' },
            { label: '關閉', value: null }
        ]
    });
    if (value === 'share' && instance) await shareScreenshot(instance, shot);
    else if (value === 'bg') await saveSettings({ background: await api.set_media({ kind: 'background', path: shot.path }) }).then(() => toast('已設為啟動器背景'), fail);
    else if (value === 'open') api.open_path({ path: shot.path }).catch(fail);
    else if (value === 'delete' && await confirm(`確定要刪除這張截圖嗎？\n${shot.name}\n${RECYCLE_NOTE}`, '刪除')) {
        await api.delete_screenshot({ id: shot.instance, file: shot.name }).then(() => {
            shots = shots.filter((s) => s !== shot);
            renderGrid();
            toast('已刪除');
        }, fail);
    }
}

const THUMB_WIDTH = 480;
const thumbQueue = [];
let thumbsBusy = 0;

async function makeThumb(shot) {
    const img = await loadImage(fileSrc(shot.path));
    if (!img?.naturalWidth) return null;
    const width = Math.min(THUMB_WIDTH, img.naturalWidth);
    const canvas = h('canvas', { width, height: Math.round(img.naturalHeight * width / img.naturalWidth) });
    canvas.getContext('2d').drawImage(img, 0, 0, canvas.width, canvas.height);
    img.src = '';
    return api.save_shot_thumb({ id: shot.instance, file: shot.name, time: shot.time, data: canvas.toDataURL('image/jpeg', 0.82) });
}

function pumpThumbs() {
    while (thumbsBusy < 2 && thumbQueue.length) {
        const [shot, img] = thumbQueue.shift();
        if (!img.isConnected) continue;
        thumbsBusy++;
        makeThumb(shot)
            .then((thumb) => { shot.thumb = thumb; img.src = fileSrc(thumb || shot.path); })
            .catch(() => { img.src = fileSrc(shot.path); })
            .finally(() => { thumbsBusy--; pumpThumbs(); });
    }
}

const thumbObserver = new IntersectionObserver((entries) => {
    for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        thumbObserver.unobserve(entry.target);
        thumbQueue.push([entry.target.shot, entry.target]);
    }
    pumpThumbs();
}, { rootMargin: '400px' });

function shotImage(shot) {
    if (shot.thumb) return h('img', { src: fileSrc(shot.thumb), alt: shot.name, loading: 'lazy', decoding: 'async' });
    const img = h('img', { alt: shot.name, decoding: 'async' });
    img.shot = shot;
    thumbObserver.observe(img);
    return img;
}

function renderGrid() {
    thumbQueue.length = 0;
    const list = shots.filter((s) => !filter || s.instance === filter);
    const thisYear = new Date().getFullYear();
    let year = thisYear;
    clear($('#album-grid')).append(...(list.length
        ? byMonth(list).flatMap((group) => {
            const head = [];
            if (group.year !== year) {
                year = group.year;
                head.push(h('div', { class: 'album-year' }, `${group.year}`));
            }
            head.push(h('div', { class: 'album-month' }, h('b', {}, `${group.month} 月`), h('span', {}, `${group.year === thisYear ? '' : `${group.year} 年・`}${group.shots.length} 張`)));
            return [...head, ...group.shots.map((shot) => h('button', { type: 'button', class: 'shot shot-open', title: `${shot.instanceName}\n${shot.name}`, onclick: () => viewShot(shot) },
                shotImage(shot),
                h('div', { class: 'shot-foot' }, h('span', {}, shot.instanceName), `${new Date(shot.time * 1000).getDate()} 日`)))];
        })
        : [h('div', { class: 'empty-state' }, '還沒有截圖。遊戲中按 F2 就能截圖，所有實例的截圖都會出現在這裡。')]));
}

async function render() {
    const now = Math.floor(Date.now() / 1000);
    const [sessions, list] = await Promise.all([api.play_history({ since: now - 14 * DAY }).catch(() => []), api.list_all_screenshots().catch(() => [])]);
    shots = list;
    const recap = weekRecap(sessions, shots, now);
    clear($('#album-recap')).append(recapCard(recap, []));
    const picks = memories(shots, now);
    $('#album-memories').hidden = !picks.length;
    clear($('#album-memories-list')).append(...picks.map(memoryCard));
    brilliantDelta(now).then((delta) => {
        if (delta.length) clear($('#album-recap')).append(recapCard(recap, delta));
    });
    const names = [...new Map(shots.map((s) => [s.instance, s.instanceName])).entries()];
    if (filter && !names.some(([id]) => id === filter)) filter = '';
    clear($('#album-filter')).append(h('option', { value: '' }, `全部實例（${shots.length}）`),
        ...names.map(([id, name]) => h('option', { value: id, selected: id === filter }, `${name}（${shots.filter((s) => s.instance === id).length}）`)));
    renderGrid();
}

export function initAlbum() {
    $('#album-filter').addEventListener('change', (e) => {
        filter = e.target.value;
        renderGrid();
    });
    $('#btn-album-refresh').addEventListener('click', guard(render));
    addEventListener('hashchange', () => { if (location.hash === '#album') render(); });
    on('instances', () => { if (document.body.dataset.view === 'album') render(); });
    watchFolders(['screenshots'], () => (location.hash === '#album' ? null : undefined), render);
    if (location.hash === '#album') render();
}
