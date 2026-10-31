import { api, fileSrc, listen, openUrl, pickFile } from '../api.js';
import { $, acceptFiles, clear, confirm, dialog, dismissDialog, fail, h, toast, guard, RECYCLE_NOTE } from '../ui.js';
import { icon } from '../icons.js';
import { avatarUrl, emit, loadAccounts, loadInstances, on, refreshAvatar, selectedAccount, store } from '../store.js';
import { provide } from '../commands.js';

const profiles = new Map();

export async function loginMicrosoft(mode) {
    if (store.online === false) {
        toast('目前沒有網路連線，連上網路後才能登入微軟帳號', 'warn');
        return;
    }
    if (mode === 'device') return loginDevice();
    const browser = mode !== false;
    let account;
    let waiting = browser;
    let switchTo = '';
    if (browser) {
        dialog({
            title: '在瀏覽器登入微軟帳號',
            content: h('div', {},
                h('p', {}, '已經用你的預設瀏覽器打開微軟登入頁。瀏覽器若已登入過微軟帳號，點一下帳號就好；也可以用密碼、手機驗證、指紋、Windows Hello 或通行金鑰登入。'),
                h('p', { class: 'hint' }, '完成後會自動回到啟動器，不用複製任何東西。沒看到瀏覽器跳出來的話，可以改用內嵌視窗登入。')),
            buttons: [{ label: '取消', value: 'cancel' }, { label: '改用代碼登入', value: 'device' }, { label: '改用內嵌視窗登入', value: 'window' }]
        }).then((value) => {
            if (!waiting) return;
            waiting = false;
            switchTo = value;
            api.cancel_login().catch(() => {});
        });
    }
    try {
        account = await api.login_microsoft({ browser });
    } catch (error) {
        if (switchTo === 'window') return loginMicrosoft(false);
        if (switchTo === 'device') return loginDevice();
        if (waiting) {
            waiting = false;
            dismissDialog(null);
        }
        return String(error?.message ?? error).includes('已取消登入') ? toast('已取消登入', 'warn') : fail(error);
    }
    if (waiting) {
        waiting = false;
        dismissDialog(null);
    }
    await loadAccounts();
    emit('settings');
    toast(`已登入：${account.name}`);
    return account;
}

async function loginDevice() {
    if (store.online === false) {
        toast('目前沒有網路連線，連上網路後才能登入微軟帳號', 'warn');
        return;
    }
    let waiting = true;
    let link = 'https://www.microsoft.com/link';
    const code = h('code', { class: 'device-code' }, '取得代碼中⋯');
    const copy = h('button', { type: 'button', class: 'mc', disabled: true, onclick: () => navigator.clipboard.writeText(code.textContent).then(() => toast('已複製代碼'), fail) }, icon('copy'), '複製代碼');
    const open = h('button', { type: 'button', class: 'mc primary', disabled: true, onclick: () => openUrl(link) }, icon('external-link'), '開啟 microsoft.com/link');
    const stop = await listen('launcher://device-code', (info) => {
        code.textContent = info.userCode;
        link = info.verificationUri || link;
        copy.disabled = false;
        open.disabled = false;
    });
    dialog({
        title: '用代碼登入微軟帳號',
        content: h('div', {},
            h('p', {}, '在手機或任何一台電腦打開 microsoft.com/link，輸入下面的代碼，再登入你的微軟帳號。'),
            h('div', { class: 'device-row' }, code),
            h('div', { class: 'toolbar device-actions' }, copy, open),
            h('p', { class: 'hint' }, '代碼 15 分鐘內有效。完成後啟動器會自動接續，不用回來按任何按鈕。')),
        buttons: [{ label: '取消', value: 'cancel' }]
    }).then(() => {
        if (!waiting) return;
        waiting = false;
        api.cancel_login().catch(() => {});
    });
    let account;
    try {
        account = await api.login_device();
    } catch (error) {
        if (waiting) {
            waiting = false;
            dismissDialog(null);
        }
        return String(error?.message ?? error).includes('已取消登入') ? toast('已取消登入', 'warn') : fail(error);
    } finally {
        stop();
    }
    if (waiting) {
        waiting = false;
        dismissDialog(null);
    }
    await loadAccounts();
    emit('settings');
    toast(`已登入：${account.name}`);
    return account;
}

export async function addOffline() {
    const input = h('input', { type: 'text', maxlength: '16', placeholder: 'Steve' });
    const ok = await dialog({
        title: '新增離線帳號',
        content: h('div', { class: 'field' },
            h('label', {}, '遊戲名稱'), input,
            h('div', { class: 'hint' }, '3～16 個英數字或底線。離線帳號無法登入正版驗證的伺服器。')),
        buttons: [{ label: '取消', value: false }, { label: '新增', value: true }]
    });
    if (ok !== true) return;
    try {
        const account = await api.add_offline_account({ name: input.value });
        await loadAccounts();
        emit('settings');
        toast(`已新增離線帳號：${account.name}`);
    } catch (error) {
        fail(error);
    }
}

export async function select(account) {
    await api.select_account({ id: account.id }).catch(fail);
    store.settings.selectedAccount = account.id;
    emit('accounts');
    emit('settings');
    await refreshSessions();
}

async function remove(account) {
    const bound = store.instances.filter((i) => i.account === account.id);
    const note = bound.length ? `\n${bound.map((i) => `「${i.name}」`).join('、')}指定用這個帳號啟動，移除後會改用目前選擇的帳號。` : '';
    if (!(await confirm(`確定要移除「${account.name}」嗎？${note}`, '移除'))) return;
    try {
        await api.remove_account({ id: account.id });
    } catch (error) {
        return fail(error);
    }
    for (const instance of bound) await api.update_instance({ updated: { ...instance, account: '' } }).catch(fail);
    if (bound.length) await loadInstances().catch(() => {});
    await loadAccounts();
    emit('settings');
}

function skinParts(side, a) {
    return side === 'back'
        ? [
            [24, 8, 8, 8, 56, 8, 4, 0],
            [32, 20, 8, 12, 32, 36, 4, 8],
            [48 + a, 20, a, 12, 48 + a, 36, 12, 8],
            [40 + a, 52, a, 12, 56 + a, 52, 4 - a, 8, [48 + a, 20]],
            [12, 20, 4, 12, 12, 36, 8, 20],
            [28, 52, 4, 12, 12, 52, 4, 20, [12, 20]]
        ]
        : [
            [8, 8, 8, 8, 40, 8, 4, 0],
            [20, 20, 8, 12, 20, 36, 4, 8],
            [44, 20, a, 12, 44, 36, 4 - a, 8],
            [36, 52, a, 12, 52, 52, 12, 8, [44, 20]],
            [4, 20, 4, 12, 4, 36, 4, 20],
            [20, 52, 4, 12, 4, 52, 8, 20, [4, 20]]
        ];
}

function drawSkin(canvas, url, { slim = false, cape = null, side = 'front' } = {}) {
    canvas.width = 16;
    canvas.height = 32;
    const ctx = canvas.getContext('2d');
    ctx.imageSmoothingEnabled = false;
    const img = new Image();
    img.onload = () => {
        const legacy = img.height === 32;
        let hat = !legacy;
        if (legacy) {
            try {
                const probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
                probe.canvas.width = 32;
                probe.canvas.height = 16;
                probe.drawImage(img, 32, 0, 32, 16, 0, 0, 32, 16);
                const alpha = probe.getImageData(0, 0, 32, 16).data;
                for (let i = 3; i < alpha.length; i += 4) if (alpha[i] < 128) { hat = true; break; }
            } catch {
                hat = false;
            }
        }
        const blit = (sx, sy, w, ht, dx, dy, flip) => {
            if (!flip) return ctx.drawImage(img, sx, sy, w, ht, dx, dy, w, ht);
            ctx.save();
            ctx.translate(dx + w, dy);
            ctx.scale(-1, 1);
            ctx.drawImage(img, sx, sy, w, ht, 0, 0, w, ht);
            ctx.restore();
        };
        skinParts(side, slim ? 3 : 4).forEach(([sx, sy, w, ht, ox, oy, dx, dy, mirror], index) => {
            if (legacy && mirror) blit(mirror[0], mirror[1], w, ht, dx, dy, true);
            else blit(sx, sy, w, ht, dx, dy, false);
            if (index === 0 ? hat : !legacy) blit(ox, oy, w, ht, dx, dy, false);
        });
        if (cape && side === 'back') {
            const capeImg = new Image();
            capeImg.onload = () => ctx.drawImage(capeImg, 1, 1, 10, 16, 3, 8, 10, 16);
            capeImg.src = cape;
        }
    };
    img.src = url;
}

function skinFigure(url, { slim = false, cape = null, back = true } = {}) {
    const views = (back ? ['front', 'back'] : ['front']).map((side) => {
        const canvas = h('canvas', { class: 'skin-canvas' });
        drawSkin(canvas, url, { slim, cape, side });
        return canvas;
    });
    return h('div', { class: 'skin-figure' }, ...views);
}

function capePreview(url) {
    const canvas = h('canvas', { class: 'cape-canvas' });
    canvas.width = 10;
    canvas.height = 16;
    const ctx = canvas.getContext('2d');
    ctx.imageSmoothingEnabled = false;
    const img = new Image();
    img.onload = () => ctx.drawImage(img, 1, 1, 10, 16, 0, 0, 10, 16);
    img.src = url;
    return canvas;
}

const https = (url) => String(url || '').replace(/^http:/, 'https:');

const ANIMATIONS = [
    ['idle', '待機', 'IdleAnimation'],
    ['walk', '走路', 'WalkingAnimation'],
    ['run', '跑步', 'RunningAnimation'],
    ['wave', '揮手', 'WaveAnimation'],
    ['crouch', '蹲下', 'CrouchAnimation'],
    ['fly', '飛行', 'FlyingAnimation']
];
let viewerLib = null;
let viewer = null;
let animationKey = 'walk';
let stageControl = null;

const loadViewer = () => viewerLib ??= new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = 'vendor/skinview3d.bundle.js';
    script.onload = () => resolve(window.skinview3d);
    script.onerror = () => {
        viewerLib = null;
        reject(new Error('3D 預覽載入失敗'));
    };
    document.head.append(script);
});

function disposeViewer() {
    viewer?.dispose();
    viewer = null;
    stageControl = null;
}

function stage(seq, { skin, slim, cape, name, fallback }) {
    const holder = h('div', { class: 'stage3d' }, fallback);
    const chips = h('div', { class: 'stage-anims' });
    const caption = h('div', { class: 'stage-caption' });
    loadViewer().then((lib) => {
        if (seq !== panelSeq) return;
        disposeViewer();
        const canvas = h('canvas');
        holder.replaceChildren(canvas);
        viewer = new lib.SkinViewer({ canvas, width: 240, height: 320, skin, model: slim ? 'slim' : 'default' });
        if (cape) viewer.loadCape(cape);
        viewer.fov = 45;
        viewer.zoom = 0.8;
        viewer.autoRotate = true;
        viewer.autoRotateSpeed = 0.5;
        viewer.controls.enableZoom = false;
        viewer.controls.addEventListener('start', () => { if (viewer) viewer.autoRotate = false; });
        viewerName = name;
        if (name && !store.settings?.streamerMode) viewer.nameTag = name;
        viewer.renderPaused = document.body.dataset.view !== 'accounts';
        const play = (key) => {
            animationKey = key;
            const Animation = lib[ANIMATIONS.find((a) => a[0] === key)[2]];
            viewer.animation = Animation ? new Animation() : null;
            clear(chips).append(...ANIMATIONS.map(([k, label]) =>
                h('button', { type: 'button', class: 'chip', 'aria-pressed': String(k === animationKey), onclick: () => play(k) }, label)));
        };
        play(animationKey);
        const restore = () => {
            viewer?.loadSkin(skin, { model: slim ? 'slim' : 'default' });
            viewer?.loadCape(cape || null);
            for (const b of document.querySelectorAll('.cape.trying')) b.classList.remove('trying');
            clear(caption);
        };
        stageControl = {
            restore,
            tryCape: (account, item) => {
                viewer.loadCape(item ? https(item.url) : null);
                viewer.autoRotate = true;
                clear(caption).append(
                    h('span', {}, `試穿披風：${item ? item.alias : '不顯示'}`),
                    h('button', { type: 'button', class: 'mc primary', onclick: () => setCape(account, item) }, '換上'),
                    h('button', { type: 'button', class: 'mc', onclick: restore }, '還原'));
            },
            tryOn: async (account, entry) => {
                try {
                    viewer.loadSkin(await api.skin_data({ file: entry.file }), { model: entry.variant === 'slim' ? 'slim' : 'default' });
                } catch (error) {
                    return fail(error);
                }
                clear(caption).append(
                    h('span', {}, `試穿中：${entry.name || '未命名'}`),
                    account?.kind === 'msa' ? h('button', { type: 'button', class: 'mc primary', onclick: () => applySkin(account, entry) }, '換上') : null,
                    h('button', { type: 'button', class: 'mc', onclick: restore }, '還原'));
            }
        };
    }).catch(() => {});
    return h('div', { class: 'look-stage' }, holder, chips, caption);
}

async function loadProfile(account) {
    if (!profiles.has(account.id)) profiles.set(account.id, await api.account_profile({ id: account.id }));
    return profiles.get(account.id);
}

async function applySkin(account, entry) {
    try {
        await api.skin_apply({ id: account.id, file: entry.file });
        profiles.delete(account.id);
        refreshAvatar(account);
        toast(`已換上「${entry.name}」，下次進入伺服器時其他人就會看到`);
    } catch (error) {
        fail(error);
    }
    renderPanel();
}

async function uploadSkin(account, dropped = null) {
    const path = dropped ?? await pickFile('skin').catch(fail);
    if (!path) return;
    const name = h('input', { type: 'text', maxlength: '32', placeholder: '例如：夏日泳裝' });
    const variant = h('select', {},
        h('option', { value: 'classic' }, '一般手臂（4 像素，Steve）'),
        h('option', { value: 'slim' }, '細手臂（3 像素，Alex）'));
    const canWear = account?.kind === 'msa';
    const wear = h('input', { type: 'checkbox', checked: canWear });
    const ok = await dialog({
        title: '加入皮膚',
        content: h('div', {},
            h('div', { class: 'field' }, h('label', {}, '名稱'), name),
            h('div', { class: 'field' }, h('label', {}, '手臂粗細'), variant),
            canWear ? h('label', {}, wear, '加入衣櫃後立刻換上') : ''),
        buttons: [{ label: '取消', value: false }, { label: '加入衣櫃', value: true }]
    });
    if (ok !== true) return;
    try {
        const entry = await api.skin_add({ path, name: name.value, variant: variant.value });
        toast(`已把「${entry.name}」加入衣櫃`);
        if (canWear && wear.checked) return applySkin(account, entry);
    } catch (error) {
        fail(error);
    }
    renderPanel();
}

async function editSkin(entry) {
    const name = h('input', { type: 'text', maxlength: '32', value: entry.name });
    const variant = h('select', {},
        h('option', { value: 'classic', selected: entry.variant !== 'slim' }, '一般手臂（Steve）'),
        h('option', { value: 'slim', selected: entry.variant === 'slim' }, '細手臂（Alex）'));
    const ok = await dialog({
        title: '編輯皮膚',
        content: h('div', {},
            h('div', { class: 'field' }, h('label', {}, '名稱'), name),
            h('div', { class: 'field' }, h('label', {}, '手臂粗細'), variant)),
        buttons: [{ label: '取消', value: false }, { label: '儲存', value: true }]
    });
    if (ok === true) await api.skin_rename({ file: entry.file, name: name.value, variant: variant.value }).catch(fail);
    renderPanel();
}

async function removeSkin(entry) {
    if (await confirm(`要把「${entry.name}」從衣櫃移除嗎？\n皮膚圖檔${RECYCLE_NOTE}`, '移除')) await api.skin_remove({ file: entry.file }).then(() => toast(`已移除「${entry.name}」`), fail);
    renderPanel();
}

async function changeName(account, profile) {
    const input = h('input', { type: 'text', maxlength: '16', value: profile.name });
    const status = h('div', { class: 'hint' }, '讀取改名資格⋯');
    let timer = 0;
    const check = async () => {
        const name = input.value.trim();
        try {
            const res = await api.name_status({ id: account.id, name: name === profile.name ? null : name });
            const since = res.info?.changedAt ? `上次改名：${new Date(res.info.changedAt).toLocaleDateString('zh-TW')}。` : '';
            const cooldown = res.info?.nameChangeAllowed === false ? '目前還在 30 天冷卻期內，暫時不能改名。' : '';
            const avail = { AVAILABLE: '這個名稱可以使用。', DUPLICATE: '這個名稱已經有人用了。', NOT_ALLOWED: '這個名稱不被允許。', INVALID: '名稱只能用 3～16 個英數字或底線。' }[res.available] ?? '';
            status.textContent = `${since}${cooldown}${avail}` || '輸入新名稱後會自動檢查能不能用。';
        } catch (error) {
            status.textContent = String(error?.message ?? error);
        }
    };
    input.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(check, 500); });
    check();
    const ok = await dialog({
        title: '更改遊戲名稱',
        content: h('div', { class: 'field' }, h('label', {}, '新名稱'), input, status,
            h('div', { class: 'hint' }, 'Mojang 規定每 30 天只能改一次名稱。')),
        buttons: [{ label: '取消', value: false }, { label: '確定改名', value: true }]
    });
    clearTimeout(timer);
    const name = input.value.trim();
    if (ok === true && name !== profile.name) {
        try {
            await api.name_change({ id: account.id, name });
            profiles.delete(account.id);
            await loadAccounts();
            toast(`已改名為 ${name}`);
        } catch (error) {
            fail(error);
        }
    }
    renderPanel();
}

function sessionLine(account) {
    if (account.kind !== 'msa') return h('div', { class: 'session' }, icon('user'), '離線帳號不需要登入驗證');
    if (account.sessionError) {
        return h('div', { class: 'session bad' }, icon('warning-box'), '登入已失效，自動續期失敗',
            h('button', { type: 'button', class: 'mc primary', onclick: loginMicrosoft }, '重新登入'));
    }
    const hours = Math.round((account.expiresAt * 1000 - Date.now()) / 3_600_000);
    return h('div', { class: 'session ok' }, icon('check'),
        hours > 0 ? `已登入，啟動器會自動續期（目前憑證約剩 ${hours} 小時）` : '已登入，憑證即將自動續期');
}

function wardrobeItem(account, entry) {
    return h('div', { class: 'wardrobe-item' },
        h('button', {
            type: 'button',
            class: 'try-on',
            title: '在上方預覽試穿',
            onclick: () => stageControl ? stageControl.tryOn(account, entry) : toast('正版帳號才能在 3D 預覽中試穿', 'warn')
        }, skinFigure(fileSrc(entry.path), { slim: entry.variant === 'slim', back: false })),
        h('div', { class: 'wardrobe-name', title: entry.name }, entry.name || '未命名'),
        h('div', { class: 'wardrobe-meta' }, entry.variant === 'slim' ? '細手臂' : '一般手臂'),
        h('div', { class: 'wardrobe-actions' },
            account?.kind === 'msa' ? h('button', { type: 'button', class: 'mc primary', onclick: () => applySkin(account, entry) }, '換上') : null,
            h('button', { type: 'button', class: 'mc icon-btn', title: '編輯', 'aria-label': '編輯', onclick: () => editSkin(entry) }, icon('edit')),
            h('button', { type: 'button', class: 'mc icon-btn danger', title: '移除', 'aria-label': '移除', onclick: () => removeSkin(entry) }, icon('trash'))));
}

async function setCape(account, cape) {
    try {
        await api.cape_set({ id: account.id, cape: cape?.id ?? null });
        profiles.delete(account.id);
        toast(cape ? `已換上「${cape.alias}」披風` : '已隱藏披風');
    } catch (error) {
        fail(error);
    }
    renderPanel();
}

let panelSeq = 0;
async function renderPanel() {
    const seq = ++panelSeq;
    const account = selectedAccount();
    const box = clear($('#account-panel'));
    disposeViewer();
    if (!account) return;
    const library = await api.skin_library().catch(() => []);
    if (seq !== panelSeq) return;
    const wardrobe = h('div', { class: 'wardrobe' },
        acceptFiles(h('button', { type: 'button', class: 'wardrobe-item wardrobe-add', title: '點一下選擇，或把皮膚 PNG 拖到這裡', onclick: () => uploadSkin(account) }, icon('plus'), '加入皮膚'), 'skin', (path) => uploadSkin(account, path)),
        ...library.map((entry) => wardrobeItem(account, entry)));
    const wardrobeSection = [h('h2', {}, '衣櫃'), h('p', { class: 'hint' }, '點皮膚可以先在上方 3D 預覽試穿，滿意再換上。'), wardrobe];

    if (account.kind !== 'msa') {
        box.append(h('h2', {}, account.name), sessionLine(account),
            h('p', { class: 'hint' }, '離線帳號的皮膚無法同步到伺服器。改用微軟正版登入，就能在這裡一鍵換皮膚、披風與名稱。'),
            ...wardrobeSection);
        return;
    }

    box.append(h('div', { class: 'empty-state' }, '讀取外觀⋯'));
    let profile;
    try {
        profile = await loadProfile(account);
    } catch (error) {
        if (seq !== panelSeq) return;
        clear(box).append(h('h2', {}, account.name), sessionLine(account),
            h('div', { class: 'empty-state' }, String(error?.message ?? error)), ...wardrobeSection);
        return;
    }
    if (seq !== panelSeq) return;

    const skin = (profile.skins || []).find((s) => s.state === 'ACTIVE');
    const capes = profile.capes || [];
    const activeCape = capes.find((c) => c.state === 'ACTIVE');
    const capeButton = (cape) => h('button', {
        type: 'button',
        class: 'cape',
        'aria-pressed': String(cape ? cape === activeCape : !activeCape),
        title: cape ? cape.alias : '不顯示披風',
        onclick: async (e) => {
            const applied = cape ? cape === activeCape : !activeCape;
            if (stageControl) {
                if (applied) return stageControl.restore();
                for (const b of document.querySelectorAll('.cape.trying')) b.classList.remove('trying');
                e.currentTarget.classList.add('trying');
                return stageControl.tryCape(account, cape);
            }
            if (!applied && await confirm(cape ? `要換上「${cape.alias}」披風嗎？` : '要隱藏披風嗎？', '換上')) setCape(account, cape);
        }
    }, cape ? capePreview(https(cape.url)) : icon('close'), h('span', {}, cape ? cape.alias : '不顯示'));

    clear(box).append(
        h('div', { class: 'look' },
            skin
                ? stage(seq, {
                    skin: https(skin.url),
                    slim: skin.variant === 'SLIM',
                    cape: activeCape ? https(activeCape.url) : null,
                    name: profile.name,
                    fallback: skinFigure(https(skin.url), { slim: skin.variant === 'SLIM', cape: activeCape ? https(activeCape.url) : null })
                })
                : h('div', { class: 'look-figure' }, h('div', { class: 'empty-state' }, '預設皮膚')),
            h('div', { class: 'look-info' },
                h('div', { class: 'look-name' }, profile.name,
                    h('button', { type: 'button', class: 'mc icon-btn', title: '更改名稱', 'aria-label': '更改名稱', onclick: () => changeName(account, profile) }, icon('edit'))),
                sessionLine(account),
                h('div', { class: 'toolbar' },
                    acceptFiles(h('button', { type: 'button', class: 'mc primary', onclick: () => uploadSkin(account) }, icon('plus'), '上傳新皮膚'), 'skin', (path) => uploadSkin(account, path)),
                    h('button', {
                        type: 'button',
                        class: 'mc',
                        onclick: async () => {
                            try {
                                const entry = await api.skin_save_current({ id: account.id });
                                toast(`已把目前皮膚存成「${entry.name}」`);
                            } catch (error) {
                                fail(error);
                            }
                            renderPanel();
                        }
                    }, icon('save'), '存到衣櫃'),
                    h('button', {
                        type: 'button',
                        class: 'mc',
                        onclick: async () => {
                            if (await confirm('要換回預設皮膚嗎？目前的皮膚可以先「存到衣櫃」。', '換回預設')) {
                                await api.skin_reset({ id: account.id }).catch(fail);
                                profiles.delete(account.id);
                                refreshAvatar(account);
                            }
                            renderPanel();
                        }
                    }, '換回預設')),
                h('div', { class: 'field' }, h('label', {}, `披風（${capes.length} 件）`),
                    capes.length ? h('div', { class: 'capes' }, capeButton(null), ...capes.map(capeButton)) : h('div', { class: 'hint' }, '這個帳號還沒有披風。')))),
        ...wardrobeSection);
}

function card(account) {
    const active = selectedAccount()?.id === account.id;
    return h(
        'div',
        { class: `card ${active ? 'selected' : ''}`, onclick: () => select(account) },
        h('img', {
            class: 'avatar',
            src: avatarUrl(account),
            'data-avatar': account.id,
            alt: '',
            style: 'width:48px;height:48px',
            onerror: (e) => { e.target.src = 'assets/icon.png'; }
        }),
        h('div', { class: 'body' },
            h('div', { class: 'title' },
                account.name,
                h('span', { class: `badge ${account.kind === 'msa' ? '' : 'grey'}` }, account.kind === 'msa' ? '正版' : '離線'),
                account.sessionError ? h('span', { class: 'badge warn' }, '需重新登入') : null),
            h('div', { class: 'meta' }, account.uuid),
            h('div', { class: 'actions' },
                h('button', {
                    type: 'button',
                    class: 'mc',
                    onclick: (e) => { e.stopPropagation(); remove(account); }
                }, '移除')))
    );
}

function render() {
    const box = clear($('#account-list'));
    if (!store.accounts.length) {
        box.append(h('div', { class: 'empty-state' }, '還沒有任何帳號。正版玩家請用「微軟正版登入」。'));
        clear($('#account-panel'));
        return;
    }
    box.append(...store.accounts.map(card));
    renderPanel();
}

export async function refreshSessions() {
    const results = await api.refresh_sessions().catch(() => []);
    if (results.some((r) => r.refreshed || !r.ok)) await loadAccounts().catch(() => {});
    const limited = results.find((r) => !r.ok && r.error.includes('HTTP 429'));
    if (limited) return toast(limited.error, 'warn');
    const broken = results.filter((r) => !r.ok).map((r) => store.accounts.find((a) => a.id === r.id)?.name).filter(Boolean);
    if (broken.length) toast(`${broken.join('、')} 的登入已失效，請到「帳號」頁重新登入。`, 'warn');
}

let viewerName = '';

export function initAccounts() {
    on('settings', () => {
        if (viewer) viewer.nameTag = viewerName && !store.settings?.streamerMode ? viewerName : null;
    });
    provide('selectAccount', select);
    $('#btn-login-msa').addEventListener('click', guard(loginMicrosoft));
    $('#btn-login-offline').addEventListener('click', guard(addOffline));
    on('accounts', render);
    addEventListener('hashchange', () => {
        if (viewer) viewer.renderPaused = location.hash !== '#accounts';
    });
    render();
}
