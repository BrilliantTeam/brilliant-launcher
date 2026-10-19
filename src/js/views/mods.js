import { api, listen, openUrl, pickFiles } from '../api.js';
import { $, $$, clear, confirm, dialog, fail, fmtBytes, h, toast } from '../ui.js';
import { emit, loadInstances, on, selectedInstance, store } from '../store.js';
import { icon, instanceIconName, instanceVisual } from '../icons.js';
import { selectInstance } from './instances.js';

const FABRIC_API = 'P7dR8mSH';
const KIND_LABEL = { mod: '模組', resourcepack: '資源包', shader: '光影' };
const KIND_ICON = { mod: 'zap', resourcepack: 'image', shader: 'sun' };
const MODRINTH_PATH = { mod: 'mod', resourcepack: 'resourcepack', shader: 'shader' };
const CATEGORIES = {
    mod: [['', '全部'], ['optimization', '效能優化'], ['utility', '實用工具'], ['decoration', '裝飾'], ['storage', '收納'], ['adventure', '冒險'], ['library', '前置函式庫']],
    resourcepack: [['', '全部'], ['vanilla-like', '原版風'], ['simplistic', '簡約'], ['realistic', '寫實'], ['gui', '介面'], ['tweaks', '微調'], ['16x', '16x'], ['32x', '32x']],
    shader: [['', '全部'], ['vanilla-like', '原版風'], ['semi-realistic', '半寫實'], ['realistic', '寫實'], ['cartoon', '卡通'], ['fantasy', '奇幻'], ['potato', '低配電腦']]
};
export const PRESETS = [
    {
        title: '效能優化包',
        desc: '參考 Fabulously Optimized 與 Additive 的核心組合：提升 FPS、降低記憶體與卡頓，畫面和玩法都跟原版一樣。',
        items: [
            ['mod', 'AANobbMI', 'Sodium'], ['mod', 'PtjYWJkn', 'Sodium Extra'], ['mod', 'Bh37bMuy', "Reese's Sodium Options"],
            ['mod', 'gvQqBUqZ', 'Lithium'], ['mod', 'uXXizFIs', 'FerriteCore'], ['mod', 'NNAgCjsB', 'Entity Culling'],
            ['mod', '5ZwdcRci', 'ImmediatelyFast'], ['mod', '51shyZVL', 'More Culling'], ['mod', 'LQ3K71Q1', 'Dynamic FPS'],
            ['mod', 'TjSm1wrD', 'ModernFix'], ['mod', 'mOgUt4GM', 'Mod Menu']
        ]
    },
    {
        title: '畫質增強包',
        desc: '在效能包之上補回 OptiFine 的畫面功能：連接材質、實體自訂材質與模型、手持光源，資源包效果更完整。',
        items: [
            ['mod', 'AANobbMI', 'Sodium'], ['mod', '1IjD5062', 'Continuity'], ['mod', 'BVzZfTc1', 'Entity Texture Features'],
            ['mod', '4I1XuqiY', 'Entity Model Features'], ['mod', 'yBW8D80W', 'LambDynamicLights'], ['mod', 'aXf2OSFU', 'Ok Zoomer']
        ]
    },
    {
        title: '輔助模組包',
        desc: '必裝的 BetterF3 加上常用的小幫手：看得懂的 F3、飽食度、方塊資訊、聊天頭像、按鍵搜尋，只改介面不改玩法。',
        items: [
            ['mod', '8shC1gFX', 'BetterF3'], ['mod', 'EsAfCjCV', 'AppleSkin'], ['mod', 'nvQzSEkH', 'Jade'],
            ['mod', 'aC3cM3Vq', 'Mouse Tweaks'], ['mod', 'xv94TkTM', 'Controlling'], ['mod', '2M01OLQq', 'Shulker Box Tooltip'],
            ['mod', 'Wb5oqrBJ', 'Chat Heads'], ['mod', 'smUP7V3r', 'Durability Tooltip']
        ]
    },
    {
        title: '光影包',
        desc: 'Sodium + Iris 光影載入器，附 Complementary 光影，進遊戲在「視訊設定 → 光影包」開啟。建議搭配獨立顯示卡。',
        items: [['mod', 'AANobbMI', 'Sodium'], ['mod', 'YL57xq9U', 'Iris Shaders'], ['shader', 'HVnmMxH1', 'Complementary Reimagined']]
    }
];

const state = { kind: 'mod', tab: 'explore', category: '', offset: 0, total: 0, searchSeq: 0, listSeq: 0, searchedFor: '' };
const installed = { mod: null, resourcepack: null, shader: null };
const busy = new Set();
let timer = 0;

const fmtCount = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}K` : String(n));

function instanceProblem(kind = state.kind) {
    const instance = selectedInstance();
    if (!instance) return '請先到「實例」建立並選擇一個遊戲實例。';
    if (kind === 'mod' && instance.loader !== 'fabric') return '原版實例不會載入模組，請到「實例」把載入器改成 Fabric。';
    if (kind === 'shader' && instance.loader !== 'fabric') return '光影需要 Fabric 與 Iris，請先把實例改成 Fabric 並安裝「光影包」。';
    return '';
}

function installedEntry(kind, projectId) {
    return installed[kind]?.find((item) => item.project_id === projectId) ?? null;
}

async function loadInstalled(kind) {
    const instance = selectedInstance();
    if (!instance) {
        installed[kind] = [];
        return;
    }
    installed[kind] = await api.content_list({ id: instance.id, kind }).catch((error) => {
        fail(error);
        return [];
    });
    if (kind === 'resourcepack') state.packs = await api.pack_order({ id: instance.id }).catch(() => []);
}

async function install(kind, projectId, title, quiet = false) {
    const instance = selectedInstance();
    const problem = instanceProblem(kind);
    if (problem) return toast(problem, 'warn');
    const key = `${kind}:${projectId}`;
    if (busy.has(key)) return;
    busy.add(key);
    renderAll();
    try {
        const names = await api.content_install({ id: instance.id, kind, project: projectId });
        if (!quiet) toast(names.length ? `已安裝 ${title}${names.length > 1 ? `（含前置 ${names.length - 1} 個）` : ''}` : `${title} 已經安裝過了`);
        await loadInstalled(kind);
        return true;
    } catch (error) {
        if (!quiet) fail(error);
        return false;
    } finally {
        busy.delete(key);
        renderAll();
    }
}

async function remove(kind, entry) {
    const warning = (entry.managed ? '\n這是啟動器自動安裝的項目，下次啟動遊戲時會再放回來。' : '')
        + (entry.dependents?.length ? `\n${entry.dependents.join('、')} 需要它，移除後這些模組可能無法載入。` : '');
    if (!(await confirm(`確定移除「${entry.name}」？${warning}`, '移除'))) return;
    try {
        await api.content_remove({ id: selectedInstance().id, kind, file: entry.file });
        toast(`已移除 ${entry.name}`);
        await loadInstalled(kind);
        renderAll();
    } catch (error) {
        fail(error);
    }
}

async function update(kind, entries) {
    const instance = selectedInstance();
    for (const entry of entries) {
        busy.add(`file:${entry.file}`);
    }
    renderAll();
    let done = 0;
    for (const entry of entries) {
        try {
            await api.content_update({ id: instance.id, kind, file: entry.file, update: entry.update });
            done++;
        } catch (error) {
            fail(error);
        }
        busy.delete(`file:${entry.file}`);
    }
    toast(`已更新 ${done} 個${KIND_LABEL[kind]}`);
    await loadInstalled(kind);
    renderAll();
}

function actionButton(kind, projectId, title) {
    const entry = installedEntry(kind, projectId);
    if (busy.has(`${kind}:${projectId}`)) return h('button', { type: 'button', class: 'mc', disabled: true }, '安裝中⋯');
    if (entry) {
        return h('button', {
            type: 'button',
            class: 'mc installed',
            title: '已安裝，點一下移除',
            onclick: (e) => { e.stopPropagation(); remove(kind, entry); }
        }, icon('check'), ' 已安裝');
    }
    return h('button', { type: 'button', class: 'mc primary', onclick: (e) => { e.stopPropagation(); install(kind, projectId, title); } }, '安裝');
}

function plainBody(markdown) {
    return String(markdown || '')
        .replace(/<[^>]+>/g, '')
        .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
        .replace(/\[\s*\]\([^)]*\)/g, '')
        .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
        .replace(/^\s*(-{3,}|\*{3,}|_{3,})\s*$/gm, '')
        .replace(/^[ \t]+$/gm, '')
        .replace(/^#+\s*/gm, '')
        .replace(/[*_`]{1,3}/g, '')
        .replace(/\n{3,}/g, '\n\n')
        .trim();
}

async function showDetail(kind, hit) {
    let detail;
    try {
        detail = await api.content_detail({ project: hit.project_id });
    } catch (error) {
        return fail(error);
    }
    const holder = h('div', { class: 'detail-action' });
    const refreshButton = () => clear(holder).append(actionButton(kind, detail.id, detail.title));
    refreshButton();
    const content = h('div', { class: 'detail' },
        h('div', { class: 'detail-head' },
            detail.icon_url ? h('img', { src: detail.icon_url, alt: '' }) : h('div', { class: 'mod-icon' }, icon(KIND_ICON[kind])),
            h('div', { class: 'grow' },
                h('div', { class: 'detail-desc' }, detail.description),
                h('div', { class: 'meta' }, `${fmtCount(detail.downloads)} 次下載｜${detail.categories.join('、')}`)),
            holder),
        detail.gallery.length ? h('div', { class: 'gallery' }, ...detail.gallery.map((img) => h('img', { src: img.url, alt: img.title || '', loading: 'lazy' }))) : '',
        h('pre', { class: 'detail-body' }, plainBody(detail.body)));
    const value = await dialog({
        title: detail.title,
        wide: true,
        content,
        buttons: [{ label: '在 Modrinth 查看', value: 'web' }, { label: '關閉', value: null }]
    });
    if (value === 'web') openUrl(`https://modrinth.com/${MODRINTH_PATH[kind]}/${detail.slug}`);
}

function resultCard(hit) {
    const kind = state.kind;
    return h('div', { class: 'mod-card', role: 'button', tabindex: '0', title: '查看詳細介紹', onclick: () => showDetail(kind, hit) },
        hit.icon_url ? h('img', { src: hit.icon_url, alt: '', loading: 'lazy' }) : h('div', { class: 'mod-icon' }, icon(KIND_ICON[kind])),
        h('div', { class: 'grow' },
            h('div', { class: 'name' }, hit.title),
            h('div', { class: 'meta' }, `${hit.author}｜${fmtCount(hit.downloads)} 次下載`),
            h('div', { class: 'desc' }, hit.description)),
        actionButton(kind, hit.project_id, hit.title));
}

function presetState(preset) {
    const missing = preset.items.filter(([kind, id]) => !installedEntry(kind, id));
    return { missing, running: preset.items.some(([kind, id]) => busy.has(`${kind}:${id}`)) };
}

async function installPreset(preset) {
    const { missing } = presetState(preset);
    const failed = [];
    for (const [kind, id, title] of missing) {
        if (!(await install(kind, id, title, true))) failed.push(title);
    }
    const ok = missing.length - failed.length;
    if (!failed.length) toast(`「${preset.title}」安裝完成（${ok} 個）`);
    else toast(`「${preset.title}」已安裝 ${ok} 個；${failed.join('、')} 目前還沒有支援 Minecraft ${selectedInstance()?.mcVersion} 的版本，之後再按一次一鍵安裝就會補上。`, 'warn');
}

function renderPresets() {
    const box = clear($('#presets'));
    box.hidden = state.kind !== 'mod';
    if (box.hidden) return;
    box.append(...PRESETS.map((preset) => {
        const { missing, running } = presetState(preset);
        const done = !missing.length;
        return h('div', { class: `preset ${done ? 'done' : ''}` },
            h('div', { class: 'preset-title' }, preset.title),
            h('div', { class: 'preset-desc' }, preset.desc),
            h('div', { class: 'preset-items' }, ...preset.items.map(([kind, id, title]) =>
                h('span', { class: installedEntry(kind, id) ? 'have' : '' }, installedEntry(kind, id) ? icon('check') : '', title))),
            h('button', {
                type: 'button',
                class: `mc ${done ? '' : 'primary'}`,
                disabled: done || running,
                onclick: () => installPreset(preset)
            }, running ? '安裝中⋯' : done ? '已全部安裝' : `一鍵安裝（${missing.length} 個）`));
    }));
}

function renderCategories() {
    clear($('#store-cats')).append(...CATEGORIES[state.kind].map(([value, label]) =>
        h('button', {
            type: 'button',
            class: 'chip',
            'aria-pressed': value === state.category ? 'true' : 'false',
            onclick: () => { state.category = value; renderCategories(); search(); }
        }, label)));
}

function renderResults() {
    for (const card of $$('#mod-results .mod-card')) {
        const hit = card._hit;
        if (hit) card.lastElementChild.replaceWith(actionButton(state.kind, hit.project_id, hit.title));
    }
}

async function search(reset = true) {
    const box = $('#mod-results');
    const more = $('#btn-more-mods');
    const instance = selectedInstance();
    const query = $('#mod-search').value.trim();
    if (reset) {
        state.offset = 0;
        clear(box);
    }
    more.hidden = true;
    const problem = instanceProblem();
    if (problem && state.kind === 'mod') {
        box.append(h('div', { class: 'empty-state' }, problem));
        return;
    }
    if (!instance) {
        box.append(h('div', { class: 'empty-state' }, instanceProblem()));
        return;
    }
    state.searchedFor = `${instance.id}|${instance.mcVersion}|${state.kind}|${state.category}|${$('#store-sort').value}|${query}`;
    const seq = ++state.searchSeq;
    if (reset) box.append(...Array.from({ length: 6 }, () => h('div', { class: 'mod-card skeleton' })));
    let result;
    try {
        result = await api.content_search({ id: instance.id, kind: state.kind, query, category: state.category, sort: $('#store-sort').value, offset: state.offset });
    } catch {
        if (seq === state.searchSeq) clear(box).append(h('div', { class: 'empty-state' }, '暫時連不上 Modrinth，請稍後再試。'));
        return;
    }
    if (seq !== state.searchSeq) return;
    if (reset) clear(box);
    state.total = result.total_hits;
    state.offset += result.hits.length;
    box.append(...result.hits.filter((hit) => hit.project_id !== FABRIC_API).map((hit) => {
        const card = resultCard(hit);
        card._hit = hit;
        return card;
    }));
    if (!box.childElementCount) box.append(h('div', { class: 'empty-state' }, '找不到符合的項目，換個關鍵字或分類試試。'));
    more.hidden = state.offset >= state.total;
}

const keyOf = (entry) => entry.project_id || entry.file.replace(/\.disabled$/, '');

function profileInfo(list) {
    const instance = selectedInstance();
    const profiles = instance?.modProfiles || [];
    const active = profiles.find((p) => p.name === instance?.activeProfile) || null;
    const editable = (list || []).filter((e) => !e.managed);
    const dirty = !!active && editable.some((e) => active.known.includes(keyOf(e)) && active.enabled.includes(keyOf(e)) !== e.enabled);
    return { instance, profiles, active, editable, dirty };
}

async function storeProfiles(profiles, active) {
    const instance = selectedInstance();
    await api.set_mod_profiles({ id: instance.id, profiles, active });
    instance.modProfiles = profiles;
    instance.activeProfile = active;
}

function snapshot(name, editable) {
    return { name, enabled: editable.filter((e) => e.enabled).map(keyOf), known: editable.map(keyOf) };
}

async function editProfile(profile) {
    const { profiles, editable, active } = profileInfo(installed.mod);
    const isNew = !profile;
    const selected = new Set(isNew ? [] : profile.enabled);
    const name = h('input', { type: 'text', maxlength: '24', value: profile?.name ?? '', placeholder: '例如：生存、建築、錄影' });
    const count = h('span', { class: 'toolbar-label' });
    const search = h('input', { type: 'search', placeholder: '搜尋模組⋯', class: 'pick-search' });
    const list = h('div', { class: 'pick-list' });
    const render = () => {
        const q = search.value.trim().toLowerCase();
        clear(list).append(...editable
            .filter((e) => !q || e.name.toLowerCase().includes(q) || e.file.toLowerCase().includes(q))
            .map((e) => h('label', { class: 'pick-row' },
                h('input', {
                    type: 'checkbox',
                    checked: selected.has(keyOf(e)),
                    onchange: (ev) => { ev.target.checked ? selected.add(keyOf(e)) : selected.delete(keyOf(e)); count.textContent = `已選 ${selected.size} / ${editable.length} 個`; }
                }),
                e.icon_url ? h('img', { class: 'pick-icon', src: e.icon_url, alt: '' }) : h('span', { class: 'pick-icon' }, icon('zap')),
                h('span', { class: 'pick-name' }, e.name),
                h('span', { class: 'pick-meta' }, e.version || ''))));
        count.textContent = `已選 ${selected.size} / ${editable.length} 個`;
    };
    const source = h('select', {
        onchange: (ev) => {
            selected.clear();
            const v = ev.target.value;
            if (v === 'current') editable.filter((e) => e.enabled).forEach((e) => selected.add(keyOf(e)));
            else if (v.startsWith('copy:')) (profiles.find((p) => p.name === v.slice(5))?.enabled || []).forEach((k) => selected.add(k));
            render();
        }
    },
    h('option', { value: 'blank' }, '從空白開始'),
    h('option', { value: 'current' }, '從目前啟用的模組開始'),
    ...profiles.map((p) => h('option', { value: `copy:${p.name}` }, `複製「${p.name}」`)));
    const bulk = (on) => {
        const q = search.value.trim().toLowerCase();
        editable.filter((e) => !q || e.name.toLowerCase().includes(q) || e.file.toLowerCase().includes(q))
            .forEach((e) => (on ? selected.add(keyOf(e)) : selected.delete(keyOf(e))));
        render();
    };
    search.addEventListener('input', render);
    render();
    const result = await dialog({
        title: isNew ? '新增模組組合' : `編輯組合「${profile.name}」`,
        wide: true,
        content: h('div', { class: 'profile-editor' },
            h('div', { class: 'field-row' },
                h('div', { class: 'field' }, h('label', {}, '組合名稱'), name),
                isNew ? h('div', { class: 'field' }, h('label', {}, '起點'), source) : null),
            h('div', { class: 'toolbar' },
                search,
                h('button', { type: 'button', class: 'mc', onclick: () => bulk(true) }, '全選'),
                h('button', { type: 'button', class: 'mc', onclick: () => bulk(false) }, '全不選'),
                h('span', { class: 'spacer' }),
                count),
            list,
            h('div', { class: 'hint' }, '套用組合時，勾選的模組會啟用、沒勾的會停用；啟動器管理的模組（Brilliant-Server-Mod、Fabric API）不受影響。之後新裝的模組在你把它加進組合前，會保持原本的狀態。')),
        buttons: [{ label: '取消', value: null }, { label: '儲存', value: 'save' }, { label: '儲存並套用', value: 'apply' }]
    });
    if (!result) return;
    const finalName = name.value.trim();
    if (!finalName) return toast('請輸入組合名稱', 'warn');
    if (profiles.some((p) => p.name === finalName && p !== profile)) return toast(`已經有叫「${finalName}」的組合了`, 'warn');
    const next = { name: finalName, enabled: [...selected], known: editable.map(keyOf) };
    const list2 = isNew ? [...profiles, next] : profiles.map((p) => (p === profile ? next : p));
    const activeName = active === profile && profile ? finalName : active?.name || '';
    try {
        await storeProfiles(list2, activeName);
        if (result === 'apply') return applyProfile(next);
        toast(`已儲存組合「${finalName}」（${next.enabled.length} 個模組）`);
    } catch (error) {
        fail(error);
    }
    renderAll();
}

async function exportProfile(profile) {
    const list = installed.mod || [];
    const files = list.filter((e) => !e.managed && profile.enabled.includes(keyOf(e))).map((e) => e.file);
    const api_ = list.find((e) => e.managed && e.project_id === FABRIC_API);
    if (api_) files.push(api_.file);
    try {
        const path = await api.profile_export({ id: selectedInstance().id, name: profile.name, files });
        toast(`已匯出「${profile.name}」到「下載」資料夾：${path.split(/[\\/]/).pop()}\n可匯入輝煌啟動器、Modrinth App、Prism Launcher 等啟動器。`);
        api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
    } catch (error) {
        fail(error);
    }
}

export async function importPack(targetId = null) {
    const path = await pickFiles({ multiple: false, filters: [{ name: 'Modrinth 模組包', extensions: ['mrpack'] }] }).catch(fail);
    if (!path) return;
    let target = targetId;
    if (target === null && selectedInstance()) {
        const mode = await dialog({
            title: '匯入模組包',
            content: h('div', {},
                h('p', {}, `「${path.split(/[\\/]/).pop()}」要匯入到哪裡？`),
                h('p', { class: 'hint' }, `加入目前實例「${selectedInstance().name}」時，會下載缺少的模組並建立同名組合；版本不同時請選「建立新實例」。`)),
            buttons: [{ label: '取消', value: null }, { label: '建立新實例', value: 'new' }, { label: '加入目前實例', value: 'current' }]
        });
        if (!mode) return;
        target = mode === 'current' ? selectedInstance().id : null;
    }
    toast('正在讀取模組包並下載模組⋯');
    let result;
    try {
        result = await api.mrpack_import({ path, target });
    } catch (error) {
        return fail(error);
    }
    if (result.skipped.length) toast(`有 ${result.skipped.length} 個檔案來源不受信任或格式不正確，已略過：${result.skipped.slice(0, 3).join('、')}`, 'warn');
    if (!target) {
        await api.select_instance({ id: result.instanceId }).catch(() => {});
        store.settings.selectedInstance = result.instanceId;
        await loadInstances().catch(() => {});
        emit('settings');
        toast(`已建立實例「${result.name || '模組包'}」，共 ${result.mods.length} 個模組`);
        return;
    }
    await loadInstalled('mod');
    const { profiles, editable } = profileInfo(installed.mod);
    const wanted = new Set(result.mods.map((m) => m.replace(/\.disabled$/, '')));
    const base = (result.name || path.split(/[\\/]/).pop().replace(/\.mrpack$/i, '')).slice(0, 24);
    let name = base;
    for (let n = 2; profiles.some((p) => p.name === name); n++) name = `${base.slice(0, 20)} ${n}`;
    const profile = { name, enabled: editable.filter((e) => wanted.has(e.file.replace(/\.disabled$/, ''))).map(keyOf), known: editable.map(keyOf) };
    await storeProfiles([...profiles, profile], profileInfo(installed.mod).active?.name || '').catch(fail);
    renderAll();
    if (await confirm(`已匯入「${name}」（${profile.enabled.length} 個模組）。要現在切換到這個組合嗎？`, '切換')) applyProfile(profile);
}

async function applyProfile(profile) {
    const { editable } = profileInfo(installed.mod);
    const changes = editable.filter((e) => profile.known.includes(keyOf(e)) && profile.enabled.includes(keyOf(e)) !== e.enabled);
    busy.add('profile');
    renderAll();
    let done = 0;
    for (const entry of changes) {
        try {
            await api.set_mod_enabled({ id: selectedInstance().id, file: entry.file, enabled: !entry.enabled });
            done++;
        } catch (error) {
            fail(error);
        }
    }
    await storeProfiles(profileInfo(installed.mod).profiles, profile.name).catch(fail);
    busy.delete('profile');
    await loadInstalled('mod');
    toast(done ? `已切換到「${profile.name}」，調整了 ${done} 個模組` : `已切換到「${profile.name}」`);
    renderAll();
}

async function deleteProfile(profile) {
    if (!(await confirm(`要刪除組合「${profile.name}」嗎？模組本身不會被移除。`, '刪除'))) return;
    const { profiles, active } = profileInfo(installed.mod);
    await storeProfiles(profiles.filter((p) => p !== profile), active === profile ? '' : active?.name || '').catch(fail);
    renderAll();
}

function profileBar(list) {
    if (state.kind !== 'mod' || !list?.length) return null;
    const { profiles, active, dirty, editable } = profileInfo(list);
    const working = busy.has('profile');
    return h('div', { class: 'profile-bar' },
        h('span', { class: 'profile-label' }, icon('briefcase'), '模組組合'),
        ...profiles.map((p) => h('span', { class: 'profile-chip' },
            h('button', {
                type: 'button',
                class: 'chip',
                disabled: working,
                'aria-pressed': String(p === active),
                title: `套用「${p.name}」（${p.enabled.length} 個模組）`,
                onclick: () => applyProfile(p)
            }, p.name, h('small', {}, String(p.enabled.length))),
            h('button', { type: 'button', class: 'chip-x', title: `匯出「${p.name}」（.mrpack）`, 'aria-label': `匯出「${p.name}」`, onclick: () => exportProfile(p) }, icon('download')),
            h('button', { type: 'button', class: 'chip-x', title: `編輯「${p.name}」`, 'aria-label': `編輯「${p.name}」`, onclick: () => editProfile(p) }, icon('edit')),
            h('button', { type: 'button', class: 'chip-x', title: `刪除「${p.name}」`, 'aria-label': `刪除「${p.name}」`, onclick: () => deleteProfile(p) }, icon('close')))),
        profiles.length ? null : h('span', { class: 'hint' }, '挑幾個模組組成「生存」「建築」等組合，之後一鍵切換。'),
        h('span', { class: 'spacer' }),
        dirty ? h('span', { class: 'badge warn' }, `「${active.name}」已變更`) : null,
        dirty ? h('button', {
            type: 'button',
            class: 'mc',
            onclick: async () => {
                await storeProfiles(profileInfo(installed.mod).profiles.map((p) => (p === active ? snapshot(p.name, editable) : p)), active.name).catch(fail);
                toast(`已更新組合「${active.name}」`);
                renderAll();
            }
        }, '更新組合') : null,
        h('button', { type: 'button', class: 'mc', disabled: working, title: '匯入 .mrpack 模組包', onclick: () => importPack() }, icon('download'), '匯入'),
        h('button', { type: 'button', class: 'mc', disabled: working, onclick: () => editProfile(null) }, icon('plus'), '新增組合'));
}

async function savePackOrder() {
    try {
        await api.set_pack_order({ id: selectedInstance().id, enabled: state.packs.filter((p) => p.enabled).map((p) => p.file) });
        toast('資源包順序已儲存，下次進遊戲生效');
    } catch (error) {
        fail(error);
        state.packs = await api.pack_order({ id: selectedInstance().id }).catch(() => state.packs);
    }
    renderAll();
}

function movePack(from, to) {
    if (to < 0 || to >= state.packs.length || from === to) return;
    const [item] = state.packs.splice(from, 1);
    state.packs.splice(to, 0, item);
    if (state.packs[to].enabled && to > 0 && !state.packs[to - 1].enabled) state.packs[to].enabled = false;
    savePackOrder();
}

function startPackDrag(event, from, enabledCount) {
    if (event.button !== 0 || event.target.closest('input, button')) return;
    const row = event.currentTarget;
    const rows = [...row.parentElement.querySelectorAll('.pack-row')];
    const step = row.offsetHeight + 4;
    const startY = event.clientY;
    let to = from;
    event.preventDefault();
    row.setPointerCapture(event.pointerId);
    row.classList.add('dragging');
    const move = (e) => {
        const dy = e.clientY - startY;
        to = Math.max(0, Math.min(rows.length - 1, from + Math.round(dy / step)));
        row.style.transform = `translateY(${dy}px)`;
        rows.forEach((r, i) => {
            if (r === row) return;
            const shift = from < to && i > from && i <= to ? -step : from > to && i < from && i >= to ? step : 0;
            r.style.transform = shift ? `translateY(${shift}px)` : '';
        });
    };
    const end = () => {
        row.removeEventListener('pointermove', move);
        row.removeEventListener('pointerup', end);
        row.removeEventListener('pointercancel', end);
        rows.forEach((r) => { r.style.transform = ''; });
        row.classList.remove('dragging');
        if (to === from) return;
        state.packs[from].enabled = state.packs[from].enabled || to < enabledCount;
        movePack(from, to);
    };
    row.addEventListener('pointermove', move);
    row.addEventListener('pointerup', end);
    row.addEventListener('pointercancel', end);
}

function packPanel() {
    if (state.kind !== 'resourcepack' || !state.packs?.length) return null;
    const enabledCount = state.packs.filter((p) => p.enabled).length;
    return h('div', { class: 'pack-order' },
        h('div', { class: 'pack-head' },
            h('b', {}, '資源包優先順序'),
            h('span', { class: 'hint' }, '上方的資源包優先。拖曳或用箭頭調整，勾選即啟用；變更會自動寫入遊戲設定。')),
        ...state.packs.map((pack, index) => h('div', {
            class: `pack-row${pack.enabled ? '' : ' off'}`,
            onpointerdown: (e) => startPackDrag(e, index, enabledCount)
        },
        h('span', { class: 'pack-grip', 'aria-hidden': 'true' }, '⋮⋮'),
        h('span', { class: 'pack-rank' }, pack.enabled ? String(index + 1) : '—'),
        h('span', { class: 'pack-name', title: pack.file },
            h('input', {
                type: 'checkbox',
                checked: pack.enabled,
                onchange: (e) => {
                    pack.enabled = e.target.checked;
                    const rest = state.packs.filter((p) => p !== pack);
                    const at = pack.enabled ? rest.filter((p) => p.enabled).length : rest.length;
                    rest.splice(at, 0, pack);
                    state.packs = rest;
                    savePackOrder();
                }
            }),
            pack.file.replace(/\.zip$/i, '')),
        h('button', { type: 'button', class: 'mc icon-btn', title: '往上', 'aria-label': '往上', disabled: !pack.enabled || index === 0, onclick: () => movePack(index, index - 1) }, icon('chevron-down')),
        h('button', { type: 'button', class: 'mc icon-btn down', title: '往下', 'aria-label': '往下', disabled: !pack.enabled || index >= enabledCount - 1, onclick: () => movePack(index, index + 1) }, icon('chevron-down')))));
}

const VERSION_TYPE = { release: ['正式版', ''], beta: ['測試版', 'warn'], alpha: ['開發版', 'warn'] };

function listNames(names, max = 3) {
    return names.length > max ? `${names.slice(0, max).join('、')} 等 ${names.length} 個` : names.join('、');
}

async function toggleEnabled(kind, entry) {
    if (entry.enabled && entry.dependents.length
        && !(await confirm(`「${entry.name}」是 ${listNames(entry.dependents)} 的前置模組，停用後它們可能無法載入。\n確定要停用嗎？`, '停用'))) return;
    await api.set_mod_enabled({ id: selectedInstance().id, file: entry.file, enabled: !entry.enabled }).catch(fail);
    await loadInstalled(kind);
    renderAll();
}

async function showVersions(kind, entry) {
    const instance = selectedInstance();
    let all = false;
    const list = h('div', { class: 'rows version-list' });
    const pin = h('input', {
        type: 'checkbox',
        checked: entry.pinned,
        onchange: async (e) => {
            await api.content_pin({ id: instance.id, project: entry.project_id, pinned: e.target.checked }).catch(fail);
            toast(e.target.checked ? `已鎖定「${entry.name}」，全部更新時會跳過` : `已解除鎖定「${entry.name}」`);
            await loadInstalled(kind);
            renderAll();
        }
    });
    const load = async () => {
        clear(list).append(h('div', { class: 'empty-state' }, '讀取版本清單⋯'));
        let versions = [];
        try {
            versions = await api.content_versions({ id: instance.id, kind, project: entry.project_id, all });
        } catch (error) {
            clear(list).append(h('div', { class: 'empty-state' }, String(error?.message ?? error)));
            return;
        }
        if (!versions.length) {
            clear(list).append(h('div', { class: 'empty-state' }, `沒有支援 Minecraft ${instance.mcVersion} 的版本。`));
            return;
        }
        clear(list).append(...versions.map((v) => {
            const current = v.id === entry.version_id;
            const fits = v.game_versions.includes(instance.mcVersion);
            const [typeLabel, typeClass] = VERSION_TYPE[v.version_type] ?? [v.version_type, ''];
            return h('div', { class: `row ${current ? 'current' : ''}` },
                h('div', { class: 'grow' },
                    h('div', { class: 'name' }, v.version_number,
                        h('span', { class: `tag ${typeClass}` }, typeLabel),
                        current ? h('span', { class: 'badge' }, '目前使用') : null),
                    h('div', { class: 'meta' }, [
                        v.date_published ? new Date(v.date_published).toLocaleDateString('zh-TW') : '',
                        `Minecraft ${v.game_versions.slice(-3).join('、')}${v.game_versions.length > 3 ? ' 等' : ''}`,
                        fits ? '' : '⚠ 不是這個實例的版本'
                    ].filter(Boolean).join('｜'))),
                h('button', {
                    type: 'button',
                    class: `mc ${current ? '' : 'primary'}`,
                    disabled: current || !v.file,
                    onclick: async () => {
                        if (!fits && !(await confirm(`${v.version_number} 不是給 Minecraft ${instance.mcVersion} 的版本，可能無法載入。\n仍要切換嗎？`, '切換'))) return showVersions(kind, entry);
                        try {
                            await api.content_update({ id: instance.id, kind, file: entry.file, update: v.file });
                            if (!entry.pinned) await api.content_pin({ id: instance.id, project: entry.project_id, pinned: true }).catch(() => {});
                            toast(`「${entry.name}」已切換到 ${v.version_number}，並自動鎖定版本`);
                        } catch (error) {
                            fail(error);
                        }
                        $('#dlg').close();
                        await loadInstalled(kind);
                        renderAll();
                    }
                }, current ? '使用中' : '切換'));
        }));
    };
    const showAll = h('input', { type: 'checkbox', onchange: (e) => { all = e.target.checked; load(); } });
    load();
    await dialog({
        title: `${entry.name} 的版本`,
        wide: true,
        content: h('div', {},
            h('div', { class: 'toolbar' },
                h('label', {}, pin, '鎖定目前版本（全部更新時跳過）'),
                h('span', { class: 'spacer' }),
                h('label', {}, showAll, '顯示其他 Minecraft 版本')),
            list)
    });
}

function depBanner(list) {
    if (state.kind !== 'mod' || !list?.length) return null;
    const missing = list.filter((e) => e.missing?.length);
    const conflicts = list.filter((e) => e.conflicts?.length);
    const unknown = list.filter((e) => !e.project_id && !e.managed).length;
    if (!missing.length && !conflicts.length) {
        return h('div', { class: 'dep-check ok' }, icon('check'),
            `前置檢查通過：${list.filter((e) => e.enabled).length} 個啟用中的模組依賴都齊全${unknown ? `（${unknown} 個不在 Modrinth 上，無法檢查）` : ''}`);
    }
    const needs = new Map();
    for (const entry of missing) for (const dep of entry.missing) needs.set(dep.project_id, dep);
    const fixing = busy.has('deps');
    return h('div', { class: 'dep-check bad' },
        h('div', { class: 'grow' },
            h('b', {}, icon('warning-box'), missing.length ? `${missing.length} 個模組缺少前置` : '有模組互相衝突'),
            ...missing.map((e) => h('div', {}, `${e.name} 需要 ${e.missing.map((d) => `${d.title}${d.disabled ? '（已停用）' : ''}`).join('、')}`)),
            ...conflicts.map((e) => h('div', {}, `${e.name} 與 ${e.conflicts.join('、')} 衝突，建議停用其中一個`))),
        needs.size ? h('button', {
            type: 'button',
            class: 'mc primary',
            disabled: fixing,
            onclick: async () => {
                busy.add('deps');
                renderAll();
                const instance = selectedInstance();
                let done = 0;
                for (const dep of needs.values()) {
                    const off = (installed.mod || []).find((e) => e.project_id === dep.project_id && !e.enabled);
                    try {
                        if (off) await api.set_mod_enabled({ id: instance.id, file: off.file, enabled: true });
                        else await api.content_install({ id: instance.id, kind: 'mod', project: dep.project_id });
                        done++;
                    } catch (error) {
                        fail(error);
                    }
                }
                busy.delete('deps');
                toast(`已補齊 ${done} 個前置模組`);
                await loadInstalled('mod');
                renderAll();
            }
        }, fixing ? '補齊中⋯' : `一鍵補齊（${needs.size}）`) : null);
}

function installedRow(entry) {
    const kind = state.kind;
    const updating = busy.has(`file:${entry.file}`);
    const problems = (entry.missing?.length ? 1 : 0) + (entry.conflicts?.length ? 1 : 0);
    return h('div', { class: `row ${entry.enabled ? '' : 'off'} ${problems ? 'has-problem' : ''}` },
        entry.icon_url ? h('img', { class: 'row-icon', src: entry.icon_url, alt: '' }) : h('div', { class: 'row-icon' }, icon(KIND_ICON[kind])),
        h('div', { class: 'grow' },
            h('div', { class: 'name' },
                entry.name,
                entry.managed ? h('span', { class: 'badge' }, '啟動器管理') : null,
                !entry.enabled ? h('span', { class: 'badge grey' }, '已停用') : null,
                entry.pinned ? h('span', { class: 'badge grey' }, '已鎖定版本') : null,
                entry.missing?.length ? h('span', { class: 'badge warn' }, `缺少 ${entry.missing.map((d) => d.title).join('、')}`) : null,
                entry.conflicts?.length ? h('span', { class: 'badge warn' }, `與 ${entry.conflicts.join('、')} 衝突`) : null,
                entry.update && !entry.pinned ? h('span', { class: 'badge warn' }, `可更新 ${entry.update.version}`) : null),
            h('div', { class: 'meta' }, [entry.version, fmtBytes(entry.size), entry.dependents?.length ? `被 ${listNames(entry.dependents)} 需要` : entry.file].filter(Boolean).join('｜'))),
        entry.update && !entry.pinned ? h('button', { type: 'button', class: 'mc primary', disabled: updating, onclick: () => update(kind, [entry]) }, updating ? '更新中⋯' : '更新') : null,
        entry.project_id && !entry.managed ? h('button', { type: 'button', class: 'mc', title: '切換版本或鎖定版本', onclick: () => showVersions(kind, entry) }, '版本') : null,
        kind === 'mod' ? h('button', { type: 'button', class: 'mc', onclick: () => toggleEnabled(kind, entry) }, entry.enabled ? '停用' : '啟用') : null,
        h('button', { type: 'button', class: 'mc', onclick: () => remove(kind, entry) }, '移除'));
}

function renderInstalled() {
    const box = clear($('#mod-list'));
    const list = installed[state.kind];
    const updates = (list || []).filter((entry) => entry.update && !entry.pinned);
    $('#installed-count').textContent = list ? `${list.length}${updates.length ? `・${updates.length} 可更新` : ''}` : '';
    const all = $('#btn-update-all');
    all.hidden = !updates.length;
    all.textContent = `全部更新（${updates.length}）`;
    $('#btn-add-mod').hidden = state.kind !== 'mod';
    const problem = instanceProblem();
    if (problem) return box.append(h('div', { class: 'empty-state' }, problem));
    if (!list) return box.append(h('div', { class: 'empty-state' }, '讀取中⋯'));
    if (!list.length) {
        return box.append(h('div', { class: 'empty-state' },
            `還沒有安裝任何${KIND_LABEL[state.kind]}。`,
            h('button', { type: 'button', class: 'mc primary', onclick: () => selectTab('explore') }, `去探索${KIND_LABEL[state.kind]}`)));
    }
    box.append(...[profileBar(list), packPanel(), depBanner(list), ...list.map(installedRow)].filter(Boolean));
}

async function pickInstance() {
    const current = selectedInstance()?.id;
    if (!store.instances.length) {
        location.hash = '#instances';
        return;
    }
    let chosen = null;
    await dialog({
        title: '切換實例',
        content: h('div', { class: 'target-list' }, ...store.instances.map((instance) =>
            h('button', {
                type: 'button',
                class: 'mc',
                'aria-pressed': String(instance.id === current),
                onclick: () => { chosen = instance.id; $('#dlg-foot button').click(); }
            },
            instanceVisual(instance),
            h('span', { class: 'grow' }, instance.name),
            h('span', { class: 'tag' }, `${instance.mcVersion}・${instance.loader === 'fabric' ? 'Fabric' : '原版'}`)))),
        buttons: [{ label: '取消', value: null }]
    });
    const instance = store.instances.find((i) => i.id === chosen);
    if (instance) await selectInstance(instance).catch(fail);
}

function renderHeader() {
    const instance = selectedInstance();
    clear($('#store-instance')).append(
        h('div', { class: 'target-icon' }, instance ? instanceVisual(instance) : icon('warning-box')),
        h('div', { class: 'target-text' },
            h('div', { class: 'target-label' }, '安裝到這個實例'),
            instance
                ? h('div', { class: 'target-name' }, instance.name,
                    h('span', { class: 'tag' }, `Minecraft ${instance.mcVersion}`),
                    h('span', { class: 'tag' }, instance.loader === 'fabric' ? 'Fabric' : '原版'))
                : h('div', { class: 'target-name' }, '尚未選擇實例')),
        h('button', { type: 'button', class: 'mc', onclick: pickInstance }, icon('chevron-down'), '切換實例'));
    for (const button of $$('#kind-tabs button')) button.setAttribute('aria-selected', button.dataset.kind === state.kind ? 'true' : 'false');
    for (const button of $$('.store-tabs button')) button.setAttribute('aria-selected', button.dataset.tab === state.tab ? 'true' : 'false');
    $('#store-explore').hidden = state.tab !== 'explore';
    $('#store-installed').hidden = state.tab !== 'installed';
    $('#mod-search').placeholder = `搜尋 Modrinth 上的${KIND_LABEL[state.kind]}⋯⋯`;
}

function renderAll() {
    renderHeader();
    renderPresets();
    renderResults();
    renderInstalled();
}

function selectTab(tab) {
    state.tab = tab;
    renderAll();
}

async function reloadInstalled() {
    const seq = ++state.listSeq;
    const kinds = state.kind === 'mod' ? ['mod', 'shader'] : [state.kind];
    await Promise.all(kinds.map(loadInstalled));
    if (seq === state.listSeq) renderAll();
}

function onInstanceChange() {
    const instance = selectedInstance();
    const key = `${instance?.id}|${instance?.mcVersion}|${instance?.loader}`;
    if (key === onInstanceChange.last) return;
    onInstanceChange.last = key;
    installed.mod = installed.resourcepack = installed.shader = null;
    renderAll();
    reloadInstalled();
    search();
}

async function installLocal(paths) {
    const instance = selectedInstance();
    if (!instance) return toast('請先選擇一個實例', 'error');
    const jars = paths.filter((p) => p.toLowerCase().endsWith('.jar'));
    if (!jars.length) return toast('只能安裝 .jar 模組檔', 'warn');
    try {
        const added = await api.add_mods({ id: instance.id, files: jars });
        toast(`已安裝 ${added.length} 個模組`);
        await loadInstalled('mod');
        renderAll();
    } catch (error) {
        fail(error);
    }
}

export function initMods() {
    for (const button of $$('#kind-tabs button')) {
        button.addEventListener('click', () => {
            state.kind = button.dataset.kind;
            state.category = '';
            renderCategories();
            renderAll();
            reloadInstalled();
            search();
        });
    }
    for (const button of $$('.store-tabs button')) button.addEventListener('click', () => selectTab(button.dataset.tab));
    $('#mod-search').addEventListener('input', () => {
        clearTimeout(timer);
        timer = setTimeout(() => search(), 400);
    });
    $('#store-sort').addEventListener('change', () => search());
    $('#btn-more-mods').addEventListener('click', () => search(false));
    $('#btn-refresh-mods').addEventListener('click', () => reloadInstalled());
    $('#btn-update-all').addEventListener('click', () => update(state.kind, (installed[state.kind] || []).filter((entry) => entry.update && !entry.pinned)));
    $('#btn-add-mod').addEventListener('click', async () => {
        const picked = await pickFiles({ multiple: true, filters: [{ name: 'Minecraft 模組', extensions: ['jar'] }] });
        if (picked) await installLocal(Array.isArray(picked) ? picked : [picked]);
    });
    $('#btn-open-mods').addEventListener('click', () => openFolder({ mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks' }[state.kind]));
    for (const button of $$('[data-folder]')) button.addEventListener('click', () => openFolder(button.dataset.folder));

    listen('tauri://drag-drop', (payload) => {
        if (!$('#view-mods').classList.contains('active')) return;
        installLocal(payload?.paths ?? []);
    });

    on('instances', onInstanceChange);
    on('settings', onInstanceChange);
    renderCategories();
    onInstanceChange();
}

async function openFolder(sub) {
    const instance = selectedInstance();
    if (!instance) return toast('請先選擇一個實例', 'error');
    try {
        const path = await api.open_instance_folder({ id: instance.id, sub });
        await api.open_path({ path });
    } catch (error) {
        fail(error);
    }
}
