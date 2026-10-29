import { api, fileSrc, isType, listen, openUrl, pickFile, typeLabel, watchFolders } from '../api.js';
import { $, $$, clear, confirm, dialog, dismissDialog, fail, fileZoneAt, fmtBytes, fmtDuration, h, modalOpen, toast, guard } from '../ui.js';
import { emit, isRunning, loaderLabel, loadInstances, modded, on, selectedInstance, store } from '../store.js';
import { icon, iconOf, instanceIconName, instanceVisual, svgFromFile } from '../icons.js';
import { accentStyle, editInstance, moreMenu, playInstance, selectInstance, shareNotice } from './instances.js';

async function modSnapshots() {
    const instance = selectedInstance();
    if (!instance) return;
    const list = (await api.list_mod_snapshots({ id: instance.id }).catch(() => null)) ?? [];
    if (!list.length) {
        return dialog({
            title: '模組快照',
            content: h('p', {}, store.settings?.modSnapshots ? '還沒有快照。下次更新或安裝模組前會自動存一份。' : '還沒有快照。可以到「設定 → 進階」開啟「更新或安裝模組前自動存一份快照」。'),
            buttons: [{ label: '知道了', value: null }]
        });
    }
    let chosen = list[0].key;
    const rows = h('div', { class: 'media-list' }, ...list.map((s) => h('label', { class: 'media-row' },
        h('input', { type: 'radio', name: 'snap', checked: s.key === chosen, onchange: () => { chosen = s.key; } }),
        h('span', { class: 'media-name' }, new Date(s.time * 1000).toLocaleString('zh-TW', { hour12: false }), h('small', {}, `${s.count} 個檔案・${fmtBytes(s.size)}`)))));
    const ok = await dialog({
        title: `「${instance.name}」的模組快照`,
        content: h('div', {}, rows, h('div', { class: 'hint' }, '還原後，目前的模組資料夾會移到回收資料夾，30 天內可以找回。')),
        buttons: [{ label: '取消', value: false }, { label: '還原這份快照', value: true }]
    });
    if (ok !== true) return;
    try {
        await api.restore_mod_snapshot({ id: instance.id, key: chosen });
        toast('已還原模組快照');
        await loadInstalled('mod');
    } catch (error) {
        fail(error);
    }
}
import { renderMarkdown } from '../markdown.js';

const FABRIC_API = 'P7dR8mSH';
const CF_FABRIC_API = 'cf-306612';
const SOURCE_NAME = { modrinth: 'Modrinth', curseforge: 'CurseForge' };
const CF_PATH = { mod: 'mc-mods', resourcepack: 'texture-packs', shader: 'shaders' };
const isCf = (id) => String(id || '').startsWith('cf-');
const cfCats = {};
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
            ['mod', 'AANobbMI', 'Sodium', 394468], ['mod', 'PtjYWJkn', 'Sodium Extra', 447673], ['mod', 'Bh37bMuy', "Reese's Sodium Options", 511319],
            ['mod', 'gvQqBUqZ', 'Lithium', 360438], ['mod', 'uXXizFIs', 'FerriteCore', 459857], ['mod', 'NNAgCjsB', 'Entity Culling', null],
            ['mod', '5ZwdcRci', 'ImmediatelyFast', 686911], ['mod', '51shyZVL', 'More Culling', 630104], ['mod', 'LQ3K71Q1', 'Dynamic FPS', 335493],
            ['mod', 'TjSm1wrD', 'ModernFix', 1367508], ['mod', 'mOgUt4GM', 'Mod Menu', 308702]
        ]
    },
    {
        title: '畫質增強包',
        desc: '在效能包之上補回 OptiFine 的畫面功能：連接材質、實體自訂材質與模型、手持光源，資源包效果更完整。',
        items: [
            ['mod', 'AANobbMI', 'Sodium', 394468], ['mod', '1IjD5062', 'Continuity', 531351], ['mod', 'BVzZfTc1', 'Entity Texture Features', 568563],
            ['mod', '4I1XuqiY', 'Entity Model Features', 844662], ['mod', 'yBW8D80W', 'LambDynamicLights', 393442], ['mod', 'aXf2OSFU', 'Ok Zoomer', 354047]
        ]
    },
    {
        title: '輔助模組包',
        desc: '必裝的 BetterF3 加上常用的小幫手：看得懂的 F3、飽食度、方塊資訊、聊天頭像、按鍵搜尋，只改介面不改玩法。',
        items: [
            ['mod', '8shC1gFX', 'BetterF3', 401648], ['mod', 'EsAfCjCV', 'AppleSkin', 248787], ['mod', 'nvQzSEkH', 'Jade', 324717],
            ['mod', 'aC3cM3Vq', 'Mouse Tweaks', 60089], ['mod', 'xv94TkTM', 'Controlling', 250398], ['mod', '2M01OLQq', 'Shulker Box Tooltip', 315811],
            ['mod', 'Wb5oqrBJ', 'Chat Heads', 407206], ['mod', 'smUP7V3r', 'Durability Tooltip', 511040]
        ]
    },
    {
        title: '光影包',
        desc: 'Sodium + Iris 光影載入器，附 Complementary 光影，進遊戲在「視訊設定 → 光影包」開啟。建議搭配獨立顯示卡。',
        items: [['mod', 'AANobbMI', 'Sodium', 394468], ['mod', 'YL57xq9U', 'Iris Shaders', 455508], ['shader', 'HVnmMxH1', 'Complementary Reimagined', 627557]]
    }
];

const state = { kind: 'mod', source: 'modrinth', tab: 'installed', category: '', offset: 0, total: 0, searchSeq: 0, listSeq: 0, searchedFor: '', filter: '' };
const installed = { mod: null, resourcepack: null, shader: null };
const busy = new Set();
let timer = 0;

const fmtCount = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}K` : String(n));

function instanceProblem(kind = state.kind) {
    const instance = selectedInstance();
    if (!instance) return '請先到「實例」建立並選擇一個遊戲實例。';
    if (kind === 'mod' && !modded(instance)) return '原版實例不會載入模組，請到「實例」把載入器改成 Fabric、NeoForge 或 Forge。';
    if (kind === 'shader' && !modded(instance)) return '光影需要模組載入器與 Iris（Forge 可用 Oculus 或 OptiFine），請先到「實例」換成有載入器的版本。';
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
        if (kind === 'mod') warnRisky(instance, names);
        return true;
    } catch (error) {
        if (!quiet) fail(error);
        return false;
    } finally {
        busy.delete(key);
        renderAll();
    }
}

function leftoverTree(nodes) {
    return h('ul', { class: 'tree' }, ...nodes.map((node) => h('li', {},
        h('span', { class: 'tree-row' }, icon(node.dir ? 'folder' : 'script-text'), h('span', {}, node.dir ? `${node.name}/` : node.name), h('small', {}, fmtBytes(node.size))),
        node.dir && node.children.length ? leftoverTree(node.children) : null)));
}

async function remove(kind, entry) {
    const warning = (entry.managed ? '\n這是啟動器自動安裝的項目，下次啟動遊戲時會再放回來。' : '')
        + (entry.dependents?.length ? `\n${entry.dependents.join('、')} 需要它，移除後這些模組可能無法載入。` : '');
    const instance = selectedInstance();
    const leftovers = kind === 'mod' && !entry.managed
        ? await api.mod_leftovers({ id: instance.id, file: entry.file }).catch(() => null)
        : null;
    const picks = (leftovers?.items ?? []).map((item) => ({ item, input: h('input', { type: 'checkbox' }) }));
    const ok = await dialog({
        title: '移除確認',
        content: h('div', {},
            h('p', {}, `確定移除「${entry.name}」？${warning}`),
            picks.length ? h('div', { class: 'leftovers' },
                h('p', { class: 'hint' }, `找到 ${picks.length} 個這個模組留下的設定或資料（共 ${fmtBytes(picks.reduce((sum, p) => sum + p.item.size, 0))}），預設會保留；想一起清掉的請勾選，勾選的會和模組一起移到回收資料夾，30 天內都能找回：`),
                h('div', { class: 'leftover-list' }, ...picks.map(({ item, input }) => {
                    const row = h(item.dir && item.children.length ? 'div' : 'label', { class: 'leftover', title: item.path },
                        input,
                        icon(item.dir ? 'folder' : 'script-text'),
                        h('span', { class: 'leftover-path' }, item.dir ? `${item.path}/` : item.path),
                        h('small', {}, fmtBytes(item.size)));
                    if (!item.dir || !item.children.length) return row;
                    return h('details', { class: 'leftover-tree' }, h('summary', {}, row), leftoverTree(item.children));
                }))) : null),
        buttons: [{ label: '取消', value: false }, { label: '移除', value: true }]
    });
    if (ok !== true) return;
    try {
        await api.content_remove({ id: instance.id, kind, file: entry.file });
        const chosen = picks.filter((p) => p.input.checked).map((p) => p.item.path);
        const moved = chosen.length ? await api.remove_leftovers({ id: instance.id, modId: leftovers.modId, pathsToRemove: chosen }).catch((error) => { fail(error); return 0; }) : 0;
        toast(moved ? `已移除 ${entry.name}，並清掉 ${moved} 個相關設定或資料` : `已移除 ${entry.name}`);
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

const CATEGORY_LABEL = {
    ...Object.fromEntries(Object.values(CATEGORIES).flat().filter(([value]) => value)),
    technology: '科技', magic: '魔法', 'game-mechanics': '遊戲機制', worldgen: '世界生成', equipment: '裝備', food: '食物',
    mobs: '生物', social: '社交', transportation: '交通', management: '管理', minigame: '小遊戲', cursed: '惡搞', economy: '經濟',
    combat: '戰鬥', audio: '音效', blocks: '方塊', entities: '實體', environment: '環境', fonts: '字型', items: '物品', locale: '語言',
    models: '模型', atmosphere: '大氣', bloom: '泛光', 'colored-lighting': '彩色光源', foliage: '植被', 'path-tracing': '路徑追蹤',
    pbr: 'PBR', reflections: '反射', shadows: '陰影', high: '高配', medium: '中配', low: '低配', screenshot: '截圖用', themed: '主題',
    'core-shaders': '核心光影', '8x-': '8x 以下', '48x': '48x', '64x': '64x', '128x': '128x', '256x': '256x', '512x+': '512x 以上',
    performance: '效能優化', 'utility-qol': '實用工具', 'adventure-rpg': '冒險', 'library-api': '前置函式庫', 'map-information': '地圖資訊',
    cosmetic: '外觀裝飾', 'armor-weapons-tools': '裝備', 'world-gen': '世界生成', 'server-utility': '伺服器工具', redstone: '紅石',
    'mc-miscellaneous': '其他', 'mc-addons': '模組附加', education: '教育', 'bug-fixes': '錯誤修正', 'twitch-integration': '直播互動',
    'mc-creator': 'MCreator', structures: '建築結構', 'ores-resources': '礦物資源', 'player-transport': '交通', 'processing': '加工',
    'energy': '能源', 'farming': '農業', 'dimensions': '維度', 'biomes': '生態域', 'genetics': '基因', 'automation': '自動化',
    '512x-and-higher': '512x 以上', medieval: '中世紀', modern: '現代', 'photo-realistic': '寫實', steampunk: '蒸汽龐克',
    traditional: '傳統', animated: '動態', 'font-packs': '字型', 'data-packs': '資料包', 'mod-support': '模組支援', miscellaneous: '其他',
    vanilla: '原版風', realistic: '寫實', fantasy: '奇幻', 'mc-food': '食物', creativemode: '創造模式', horror: '恐怖'
};
const LOADER_TAGS = new Set(['fabric', 'quilt', 'forge', 'neoforge', 'iris', 'optifine', 'canvas', 'vanilla', 'minecraft', 'bukkit', 'spigot', 'paper', 'purpur', 'folia', 'sponge', 'datapack', 'velocity', 'waterfall', 'bungeecord']);
const catName = (c) => CATEGORY_LABEL[c] || c;
const dateText = (iso) => (iso ? new Date(iso).toLocaleDateString('zh-TW') : '');

function ago(iso) {
    const days = (Date.now() - Date.parse(iso)) / 86_400_000;
    if (!Number.isFinite(days)) return '';
    if (days < 1) return '今天';
    if (days < 30) return `${Math.floor(days)} 天前`;
    if (days < 365) return `${Math.floor(days / 30)} 個月前`;
    return `${Math.floor(days / 365)} 年前`;
}

function sideLabel(kind, client, server) {
    if (kind !== 'mod' || !client) return '';
    if (client === 'unsupported') return '伺服器專用';
    if (server === 'unsupported') return '僅用戶端';
    if (client === 'required' && server === 'required') return '雙端都要裝';
    return client === 'optional' ? '用戶端可選' : '用戶端';
}

function compatBox(kind, detail) {
    const instance = selectedInstance();
    if (!instance) return null;
    const c = detail.compat;
    if (!c) return h('div', { class: 'compat warn' }, icon('warning-box'), `目前沒有支援 Minecraft ${instance.mcVersion} 的版本，安裝不了。`);
    const names = (type) => c.deps.filter((d) => d.kind === type);
    const required = names('required');
    const optional = names('optional');
    const incompatible = names('incompatible');
    return h('div', { class: 'compat' },
        h('div', {}, icon('check'), `支援 Minecraft ${c.mc}：最新版本 ${c.version_number}${c.version_type !== 'release' ? `（${VERSION_TYPE[c.version_type]?.[0] ?? c.version_type}）` : ''}，${dateText(c.date_published)} 發佈`),
        kind === 'mod' && required.length ? h('div', {}, icon('download'), `需要前置：${required.map((d) => d.title).join('、')}（安裝時會自動一起裝好）`) : null,
        optional.length ? h('div', {}, icon('plus'), `可搭配：${optional.map((d) => d.title).join('、')}`) : null,
        incompatible.length ? h('div', { class: 'bad' }, icon('warning-box'), `不相容：${incompatible.map((d) => `${d.title}${installedEntry(kind, d.project_id) ? '（這個實例已安裝，請擇一使用）' : ''}`).join('、')}`) : null);
}

async function showDetail(kind, hit) {
    let detail;
    try {
        detail = await api.content_detail({ project: hit.project_id, kind, id: selectedInstance()?.id ?? null });
    } catch (error) {
        return fail(error);
    }
    const holder = h('div', { class: 'detail-action' });
    const refreshButton = () => clear(holder).append(actionButton(kind, detail.id, detail.title));
    refreshButton();
    const authors = detail.authors?.length ? detail.authors : hit.author ? [hit.author] : [];
    const license = detail.license?.name || detail.license?.id || '';
    const side = sideLabel(kind, detail.client_side, detail.server_side);
    const stat = (name, text) => (text ? h('span', { class: 'dstat' }, icon(name), text) : null);
    const cf = isCf(detail.id);
    const links = [
        ['script-text', '原始碼', detail.source_url],
        ['bug', '問題回報', detail.issues_url],
        ['book', 'Wiki', detail.wiki_url],
        ['discord', 'Discord', detail.discord_url],
        ...(detail.donation_urls || []).map((d) => ['gift', `贊助 ${d.platform}`.trim(), d.url])
    ].filter(([, , url]) => /^https?:\/\//.test(url || ''));
    const panes = {
        about: () => renderMarkdown(detail.body || detail.description, openUrl),
        changelog: () => (detail.compat?.changelog ? renderMarkdown(detail.compat.changelog, openUrl) : h('p', { class: 'hint' }, detail.compat ? '這個版本沒有附上更新日誌。' : '沒有適用目前實例的版本。')),
        gallery: () => h('div', { class: 'detail-gallery' }, ...detail.gallery.map((img) => h('figure', {},
            h('img', { src: img.url, alt: img.title || '', loading: 'lazy', onclick: (e) => e.currentTarget.parentElement.classList.toggle('zoom') }),
            img.title ? h('figcaption', {}, img.title) : null)))
    };
    const pane = h('div', { class: 'detail-body' });
    const tabs = h('div', { class: 'store-tabs detail-tabs', role: 'tablist' }, ...[
        ['about', '介紹'],
        ['changelog', '更新日誌'],
        detail.gallery.length ? ['gallery', `圖片（${detail.gallery.length}）`] : null
    ].filter(Boolean).map(([key, label]) => h('button', {
        type: 'button',
        'data-tab': key,
        'aria-selected': key === 'about' ? 'true' : 'false',
        onclick: () => showPane(key)
    }, label)));
    const showPane = (key) => {
        for (const b of tabs.children) b.setAttribute('aria-selected', b.dataset.tab === key ? 'true' : 'false');
        clear(pane).append(panes[key]());
        pane.scrollTop = 0;
    };
    showPane('about');
    const content = h('div', { class: 'detail' },
        h('div', { class: 'detail-head' },
            detail.icon_url ? h('img', { src: detail.icon_url, alt: '' }) : h('div', { class: 'mod-icon' }, icon(KIND_ICON[kind])),
            h('div', { class: 'grow' },
                h('div', { class: 'detail-desc' }, detail.description),
                authors.length ? h('div', { class: 'meta' }, `作者：${authors.join('、')}`) : null),
            holder),
        h('div', { class: 'detail-stats' },
            stat('download', `${fmtCount(detail.downloads)} 次下載`),
            stat('heart', `${fmtCount(detail.followers)} ${cf ? '個讚' : '人關注'}`),
            stat('clock', detail.updated ? `${ago(detail.updated)}更新` : ''),
            stat('archive', detail.published ? `${dateText(detail.published)} 上架` : ''),
            stat('book', license ? `授權 ${license}` : ''),
            stat('user', side)),
        detail.categories.length ? h('div', { class: 'mod-tags' }, ...detail.categories.filter((c) => !LOADER_TAGS.has(c)).map((c) => h('span', { class: 'mtag' }, catName(c)))) : null,
        compatBox(kind, detail),
        tabs,
        pane,
        links.length ? h('div', { class: 'detail-links' }, ...links.map(([name, label, url]) => h('button', { type: 'button', class: 'mc', onclick: () => openUrl(url) }, icon(name), label))) : null);
    const value = await dialog({
        title: detail.title,
        wide: true,
        content,
        buttons: [{ label: `在 ${cf ? 'CurseForge' : 'Modrinth'} 查看`, value: 'web' }, { label: '關閉', value: null }]
    });
    if (value === 'web') openUrl(cf ? `https://www.curseforge.com/minecraft/${CF_PATH[kind]}/${detail.slug}` : `https://modrinth.com/${MODRINTH_PATH[kind]}/${detail.slug}`);
}

function resultCard(hit) {
    const kind = state.kind;
    const side = sideLabel(kind, hit.client_side, hit.server_side);
    const cats = (hit.display_categories || []).filter((c) => !LOADER_TAGS.has(c)).slice(0, 3);
    return h('div', { class: 'mod-card', role: 'button', tabindex: '0', title: '查看詳細介紹', onclick: () => showDetail(kind, hit) },
        hit.icon_url ? h('img', { src: hit.icon_url, alt: '', loading: 'lazy' }) : h('div', { class: 'mod-icon' }, icon(KIND_ICON[kind])),
        h('div', { class: 'grow' },
            h('div', { class: 'name' }, hit.title),
            h('div', { class: 'meta' }, [hit.author, `${fmtCount(hit.downloads)} 下載`, hit.date_modified ? `${ago(hit.date_modified)}更新` : ''].filter(Boolean).join('｜')),
            h('div', { class: 'desc' }, hit.description),
            side || cats.length ? h('div', { class: 'mod-tags' }, side ? h('span', { class: 'mtag side' }, side) : null, ...cats.map((c) => h('span', { class: 'mtag' }, catName(c)))) : null),
        actionButton(kind, hit.project_id, hit.title));
}

export function presetSources([, id, , cf], instance = selectedInstance()) {
    if (!cf) return [id];
    return instance?.contentSource === 'curseforge' ? [`cf-${cf}`, id] : [id, `cf-${cf}`];
}

const presetEntry = (item) => presetSources(item).map((project) => installedEntry(item[0], project)).find(Boolean) ?? null;

function presetState(preset) {
    const missing = preset.items.filter((item) => !presetEntry(item));
    return { missing, running: preset.items.some((item) => presetSources(item).some((project) => busy.has(`${item[0]}:${project}`))) };
}

async function installItems(preset, items) {
    const failed = [];
    for (const item of items) {
        let ok = false;
        for (const project of presetSources(item)) {
            if ((ok = await install(item[0], project, item[2], true))) break;
        }
        if (!ok) failed.push(item[2]);
    }
    const ok = items.length - failed.length;
    if (!failed.length) toast(`「${preset.title}」安裝完成（${ok} 個）`);
    else toast(`「${preset.title}」已安裝 ${ok} 個；${failed.join('、')} 目前還沒有支援 Minecraft ${selectedInstance()?.mcVersion} 的版本，之後再安裝一次就會補上。`, 'warn');
}

async function installPreset(preset) {
    await installItems(preset, presetState(preset).missing);
}

async function customizePreset(preset) {
    const rows = preset.items.map((item) => {
        const title = item[2];
        const have = !!presetEntry(item);
        const input = h('input', { type: 'checkbox', checked: !have, disabled: have });
        const el = h('label', { class: `pick-row ${have ? 'have' : ''}` }, input, h('span', { class: 'grow' }, title), have ? h('span', { class: 'pick-tag' }, icon('check'), '已安裝') : null);
        return { item, have, input, el };
    });
    const count = h('span', { class: 'pick-count' });
    const update = () => {
        count.textContent = `已選 ${rows.filter((r) => !r.have && r.input.checked).length} 個`;
    };
    const setAll = (value) => {
        for (const r of rows) if (!r.have) r.input.checked = value;
        update();
    };
    for (const r of rows) r.input.addEventListener('change', update);
    update();
    const content = h('div', { class: 'pick' },
        h('p', { class: 'hint' }, `${preset.desc}\n勾選想要的模組再安裝；需要的前置模組會自動一起裝好，已安裝的不會重複下載。`),
        h('div', { class: 'pick-tools' },
            h('button', { type: 'button', class: 'mc', onclick: () => setAll(true) }, '全選'),
            h('button', { type: 'button', class: 'mc', onclick: () => setAll(false) }, '全不選'),
            count),
        h('div', { class: 'pick-list' }, ...rows.map((r) => r.el)));
    const value = await dialog({
        title: `自選安裝：${preset.title}`,
        content,
        buttons: [{ label: '取消', value: null }, { label: '安裝所選', value: 'install' }]
    });
    if (value !== 'install') return;
    const chosen = rows.filter((r) => !r.have && r.input.checked).map((r) => r.item);
    if (!chosen.length) return toast('沒有選擇任何模組', 'warn');
    await installItems(preset, chosen);
}

function renderPresets() {
    const box = clear($('#presets'));
    box.hidden = state.kind !== 'mod' || selectedInstance()?.loader !== 'fabric';
    if (box.hidden) return;
    box.append(...PRESETS.map((preset) => {
        const { missing, running } = presetState(preset);
        const done = !missing.length;
        return h('div', { class: `preset ${done ? 'done' : ''}` },
            h('div', { class: 'preset-title' }, preset.title),
            h('div', { class: 'preset-desc' }, preset.desc),
            h('div', { class: 'preset-items' }, ...preset.items.map((item) =>
                h('span', { class: presetEntry(item) ? 'have' : '' }, presetEntry(item) ? icon('check') : '', item[2]))),
            h('div', { class: 'preset-actions' },
                h('button', {
                    type: 'button',
                    class: `mc ${done ? '' : 'primary'}`,
                    disabled: done || running,
                    onclick: () => installPreset(preset)
                }, running ? '安裝中⋯' : done ? '已全部安裝' : `一鍵安裝（${missing.length} 個）`),
                done ? null : h('button', {
                    type: 'button',
                    class: 'mc',
                    disabled: running,
                    title: '勾選只想安裝的部分模組',
                    onclick: () => customizePreset(preset)
                }, icon('sliders'), '自選')));
    }));
}

function categoryList() {
    if (state.source !== 'curseforge') return CATEGORIES[state.kind];
    const kind = state.kind;
    if (!cfCats[kind]) {
        cfCats[kind] = [];
        api.content_categories({ kind }).then((list) => {
            if (!Array.isArray(list) || !list.length) return;
            cfCats[kind] = list;
            if (state.kind === kind && state.source === 'curseforge') renderCategories();
        }).catch(() => { delete cfCats[kind]; });
    }
    return [['', '全部'], ...cfCats[kind].map((c) => [c.id, CATEGORY_LABEL[c.slug] || c.name])];
}

function renderCategories() {
    clear($('#store-cats')).append(...categoryList().map(([value, label]) =>
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
    if (reset) state.offset = 0;
    more.hidden = true;
    const problem = instanceProblem();
    if (problem && state.kind === 'mod') {
        clear(box).append(h('div', { class: 'empty-state' }, problem));
        return;
    }
    if (!instance) {
        clear(box).append(h('div', { class: 'empty-state' }, instanceProblem()));
        return;
    }
    const side = state.kind === 'mod' && state.source === 'modrinth' ? $('#store-side').value : '';
    state.searchedFor = `${instance.id}|${instance.mcVersion}|${state.source}|${state.kind}|${state.category}|${$('#store-sort').value}|${side}|${query}`;
    const seq = ++state.searchSeq;
    if (reset && box.querySelector('.mod-card:not(.skeleton)')) {
        box.classList.add('refreshing');
    } else if (reset) {
        clear(box).append(...Array.from({ length: 6 }, () => h('div', { class: 'mod-card skeleton' })));
    }
    let result;
    state.loadingMore = true;
    try {
        result = await api.content_search({ id: instance.id, kind: state.kind, query, category: state.category, sort: $('#store-sort').value, offset: state.offset, side, source: state.source });
    } catch {
        if (seq === state.searchSeq) clear(box).append(h('div', { class: 'empty-state' }, `暫時連不上 ${SOURCE_NAME[state.source]}，請稍後再試。`));
        return;
    } finally {
        if (seq === state.searchSeq) {
            state.loadingMore = false;
            box.classList.remove('refreshing');
        }
    }
    if (seq !== state.searchSeq) return;
    if (reset) clear(box);
    state.total = result.total_hits;
    state.offset += result.hits.length;
    const autoApi = !instance.general && instance.loader === 'fabric' && (instance.injectRichPresence ?? store.settings?.injectRichPresence ?? true);
    box.append(...result.hits.filter((hit) => !autoApi || (hit.project_id !== FABRIC_API && hit.project_id !== CF_FABRIC_API)).map((hit) => {
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
    const format = await dialog({
        title: `匯出「${profile.name}」`,
        content: h('div', {},
            h('p', {}, '要匯出成哪一種模組包？'),
            h('p', { class: 'hint' }, 'Modrinth（.mrpack）：可匯入輝煌啟動器、Modrinth App、Prism Launcher 等。\nCurseForge（.zip）：可匯入輝煌啟動器、CurseForge App、Prism Launcher 等。\n不在該平台上的模組會直接打包進檔案裡。')),
        buttons: [{ label: '取消', value: null }, { label: 'CurseForge（.zip）', value: 'curseforge' }, { label: 'Modrinth（.mrpack）', value: 'modrinth' }]
    });
    if (!format) return;
    try {
        const path = await api.profile_export({ id: selectedInstance().id, name: profile.name, files, format });
        toast(`已匯出「${profile.name}」到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
        api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
    } catch (error) {
        fail(error);
    }
}

async function showBlocked(list, instanceId) {
    const value = await dialog({
        title: '有模組需要手動下載',
        content: h('div', {},
            h('p', {}, `下面 ${list.length} 個項目的作者不允許第三方啟動器下載，請到 CurseForge 網頁下載後，放進實例的 mods（資源包放 resourcepacks、光影放 shaderpacks）資料夾。`),
            h('div', { class: 'rows' }, ...list.map((b) => h('div', { class: 'row' },
                h('div', { class: 'grow' }, h('div', { class: 'name' }, b.name)),
                /^https:\/\//.test(b.url) ? h('button', { type: 'button', class: 'mc', onclick: () => openUrl(b.url) }, icon('external-link'), '開啟網頁') : null)))),
        buttons: [{ label: '開啟 mods 資料夾', value: 'folder' }, { label: '知道了', value: null }]
    });
    if (value !== 'folder') return;
    try {
        await api.open_path({ path: await api.open_instance_folder({ id: instanceId, sub: 'mods' }) });
    } catch (error) {
        fail(error);
    }
}

const PACK_PARTS = [['mods', '模組'], ['resourcepacks', '資源包'], ['shaderpacks', '光影'], ['other', '設定檔與其他檔案（config 等）']];

export async function importPack(targetId = null, dropped = null) {
    const path = dropped ?? await pickFile('modpack').catch(fail);
    if (!path) return;
    const target = targetId;
    let parts = null;
    if (target) {
        const instance = store.instances.find((i) => i.id === target);
        const boxes = PACK_PARTS.map(([value, label]) => h('label', {}, h('input', { type: 'checkbox', checked: true, value }), label));
        const ok = await dialog({
            title: `匯入模組包到「${instance?.name ?? '目前實例'}」`,
            content: h('div', {},
                h('p', {}, `要從「${path.split(/[\\/]/).pop()}」匯入哪些內容？`),
                h('div', { class: 'field import-parts' }, ...boxes),
                h('p', { class: 'hint' }, '會下載缺少的檔案；被取代的檔案會先移到回收資料夾。模組會另外建立一個同名的模組組合。模組包的 Minecraft 版本與載入器要跟這個實例相同。')),
            buttons: [{ label: '取消', value: false }, { label: '開始匯入', value: true }]
        });
        if (ok !== true) return;
        parts = boxes.map((b) => b.querySelector('input')).filter((i) => i.checked).map((i) => i.value);
        if (!parts.length) return toast('沒有勾選任何內容', 'warn');
    }
    toast(target ? '正在讀取模組包並下載檔案⋯' : '正在讀取模組包並建立新實例⋯');
    let result;
    try {
        result = await api.mrpack_import({ path, target, parts });
    } catch (error) {
        return fail(error);
    }
    if (result.skipped.length) toast(`有 ${result.skipped.length} 個檔案來源不受信任或格式不正確，已略過：${result.skipped.slice(0, 3).join('、')}`, 'warn');
    if (result.blocked?.length) await showBlocked(result.blocked, result.instanceId);
    if (!target) {
        await api.select_instance({ id: result.instanceId }).catch(() => {});
        store.settings.selectedInstance = result.instanceId;
        await loadInstances().catch(() => {});
        emit('settings');
        toast(`已建立實例「${result.name || '模組包'}」，共 ${result.mods.length} 個模組`);
        return;
    }
    await Promise.all(['mod', 'resourcepack', 'shader'].map(loadInstalled));
    renderAll();
    if (!result.mods.length) return toast(`已匯入「${result.name || '模組包'}」`);
    const { profiles, editable } = profileInfo(installed.mod);
    const wanted = new Set(result.mods.map((m) => m.replace(/\.disabled$/, '')));
    const base = (result.name || path.split(/[\\/]/).pop().replace(/\.(mrpack|zip)$/i, '')).slice(0, 24);
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
            h('button', { type: 'button', class: 'chip-x', title: `匯出「${p.name}」（.mrpack 或 CurseForge .zip）`, 'aria-label': `匯出「${p.name}」`, onclick: () => exportProfile(p) }, icon('download')),
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
        h('button', { type: 'button', class: 'mc', disabled: working, title: '把模組包的內容加入這個實例（Modrinth .mrpack 或 CurseForge .zip）', onclick: () => selectedInstance() && importPack(selectedInstance().id) }, icon('download'), '匯入'),
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

function packIcon(file) {
    const url = (installed.resourcepack || []).find((e) => e.file === file)?.icon_url;
    return url ? h('img', { class: 'pack-icon', src: url, alt: '' }) : h('span', { class: 'pack-icon' }, icon(KIND_ICON.resourcepack));
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
        packIcon(pack.file),
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
            `前置檢查通過：${list.filter((e) => e.enabled).length} 個啟用中的模組依賴都齊全${unknown ? `（${unknown} 個不在 Modrinth 或 CurseForge 上，無法檢查）` : ''}`);
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

function missingOf(kind, entry) {
    const deps = entry.missing || [];
    if (kind === 'mod') return deps;
    return deps.filter((dep) => !(installed.mod || []).some((m) => m.project_id === dep.project_id && m.enabled));
}

async function installMissingMods(entry, deps) {
    const problem = instanceProblem('mod');
    if (problem) return toast(problem, 'warn');
    const key = `deps:${entry.file}`;
    busy.add(key);
    renderAll();
    const instance = selectedInstance();
    let done = 0;
    for (const dep of deps) {
        const off = (installed.mod || []).find((m) => m.project_id === dep.project_id && !m.enabled);
        try {
            if (off) await api.set_mod_enabled({ id: instance.id, file: off.file, enabled: true });
            else await api.content_install({ id: instance.id, kind: 'mod', project: dep.project_id });
            done++;
        } catch (error) {
            fail(error);
        }
    }
    busy.delete(key);
    if (done) toast(`已安裝 ${deps.map((d) => d.title).join('、')}`);
    await loadInstalled('mod');
    renderAll();
}

function installedRow(entry) {
    const kind = state.kind;
    const updating = busy.has(`file:${entry.file}`);
    const missing = missingOf(kind, entry);
    const problems = (missing.length ? 1 : 0) + (entry.conflicts?.length ? 1 : 0);
    return h('div', { class: `row ${entry.enabled ? '' : 'off'} ${problems ? 'has-problem' : ''}` },
        entry.icon_url ? h('img', { class: 'row-icon', src: entry.icon_url, alt: '' }) : h('div', { class: 'row-icon' }, icon(KIND_ICON[kind])),
        h('div', entry.project_id && !entry.managed ? { class: 'grow clickable', title: '查看詳細介紹', onclick: () => showDetail(kind, { project_id: entry.project_id, title: entry.name }) } : { class: 'grow' },
            h('div', { class: 'name' },
                entry.name,
                entry.managed ? h('span', { class: 'badge' }, '啟動器管理') : null,
                !entry.enabled ? h('span', { class: 'badge grey' }, '已停用') : null,
                entry.pinned ? h('span', { class: 'badge grey' }, '已鎖定版本') : null,
                missing.length ? h('span', { class: 'badge warn' }, `缺少 ${missing.map((d) => d.title).join('、')}`) : null,
                entry.conflicts?.length ? h('span', { class: 'badge warn' }, `與 ${entry.conflicts.join('、')} 衝突`) : null,
                entry.update && !entry.pinned ? h('span', { class: 'badge warn' }, `可更新 ${entry.update.version}`) : null),
            h('div', { class: 'meta' }, [entry.version, fmtBytes(entry.size), entry.dependents?.length ? `被 ${listNames(entry.dependents)} 需要` : entry.file].filter(Boolean).join('｜'))),
        kind !== 'mod' && missing.length ? h('button', {
            type: 'button',
            class: 'mc primary',
            disabled: busy.has(`deps:${entry.file}`),
            title: `下載 ${missing.map((d) => d.title).join('、')}`,
            onclick: () => installMissingMods(entry, missing)
        }, busy.has(`deps:${entry.file}`) ? '下載中⋯' : '下載模組') : null,
        entry.update && !entry.pinned ? h('button', { type: 'button', class: 'mc primary', disabled: updating, onclick: () => update(kind, [entry]) }, updating ? '更新中⋯' : '更新') : null,
        entry.project_id && !entry.managed ? h('button', { type: 'button', class: 'mc', title: '切換版本或鎖定版本', onclick: () => showVersions(kind, entry) }, '版本') : null,
        kind === 'mod' ? h('button', { type: 'button', class: 'mc', onclick: () => toggleEnabled(kind, entry) }, entry.enabled ? '停用' : '啟用') : null,
        h('button', { type: 'button', class: 'mc', onclick: () => remove(kind, entry) }, '移除'));
}

let noticeSeq = 0;
async function updateShareNotice() {
    const seq = ++noticeSeq;
    const folder = { resourcepack: 'resourcepacks', shader: 'shaderpacks' }[state.kind];
    const instance = selectedInstance();
    const el = folder && instance ? await shareNotice(instance, folder) : null;
    if (seq === noticeSeq) $('#share-notice').replaceChildren(...(el ? [el] : []));
}

function renderInstalled() {
    updateShareNotice();
    const box = clear($('#mod-list'));
    const list = installed[state.kind];
    const updates = (list || []).filter((entry) => entry.update && !entry.pinned);
    $('#installed-count').textContent = list ? `${list.length}${updates.length ? `・${updates.length} 可更新` : ''}` : '';
    const all = $('#btn-update-all');
    all.hidden = !updates.length;
    all.textContent = `全部更新（${updates.length}）`;
    const problem = instanceProblem();
    if (problem) return box.append(h('div', { class: 'empty-state' }, problem));
    if (!list) return box.append(h('div', { class: 'empty-state' }, '讀取中⋯'));
    if (!list.length) {
        return box.append(h('div', { class: 'empty-state' },
            `還沒有安裝任何${KIND_LABEL[state.kind]}。`,
            h('button', { type: 'button', class: 'mc primary', onclick: () => selectTab('explore') }, `去探索${KIND_LABEL[state.kind]}`)));
    }
    const q = state.filter.trim().toLowerCase();
    if (q) {
        const hits = list.filter((e) => [e.name, e.file, e.version].some((v) => (v || '').toLowerCase().includes(q)));
        return box.append(...(hits.length
            ? [h('div', { class: 'hint' }, `找到 ${hits.length} 個符合「${state.filter.trim()}」的${KIND_LABEL[state.kind]}`), ...hits.map(installedRow)]
            : [h('div', { class: 'empty-state' }, `已安裝的${KIND_LABEL[state.kind]}裡沒有「${state.filter.trim()}」。`,
                h('button', { type: 'button', class: 'mc primary', onclick: () => { $('#mod-search').value = state.filter.trim(); state.filter = ''; $('#installed-search').value = ''; selectTab('explore'); search(); } }, `到 ${SOURCE_NAME[state.source]} 搜尋`))]));
    }
    box.append(...[profileBar(list), packPanel(), depBanner(list), ...list.map(installedRow)].filter(Boolean));
}

function toggleTargetMenu(force) {
    const menu = $('#store-instance .target-menu');
    if (!menu) return;
    const open = force ?? menu.hidden;
    menu.hidden = !open;
    menu.previousElementSibling.setAttribute('aria-expanded', String(open));
    if (!open) return;
    const current = selectedInstance()?.id;
    clear(menu).append(
        ...store.instances.map((instance) => h('button', {
            type: 'button',
            role: 'menuitem',
            'aria-current': String(instance.id === current),
            onclick: async () => {
                toggleTargetMenu(false);
                if (instance.id !== current) await selectInstance(instance).catch(fail);
            }
        },
        h('span', { class: 'inst-thumb' }, instanceVisual(instance)),
        h('span', { class: 'menu-name' }, instance.name),
        h('span', { class: 'sub' }, `${instance.mcVersion}・${loaderLabel(instance)}`))),
        store.instances.length ? h('hr') : null,
        h('button', { type: 'button', role: 'menuitem', onclick: () => { toggleTargetMenu(false); location.hash = '#instances'; } }, store.instances.length ? '管理實例⋯' : '建立實例⋯')
    );
}

addEventListener('click', (e) => {
    if (!e.target.closest('.target-pick')) toggleTargetMenu(false);
});
addEventListener('keydown', (e) => {
    if (e.key === 'Escape') toggleTargetMenu(false);
});

function renderHeader() {
    const instance = selectedInstance();
    const running = instance && isRunning(instance.id);
    const cover = instance?.image || '';
    const vector = /\.svg$/i.test(cover);
    const head = $('#store-instance');
    head.classList.toggle('has-cover', !!cover);
    $('#view-mods').style.cssText = accentStyle(instance?.accent);
    clear(head).append(
        cover ? h('div', { class: 'inst-page-cover', 'aria-hidden': 'true', style: vector ? '' : `background-image:url("${fileSrc(cover)}")` }, vector ? svgFromFile(cover, 'cover-svg') : null) : null,
        h('div', { class: 'inst-thumb inst-page-icon' }, instance ? iconOf(instance.icon) : icon('warning-box')),
        h('div', { class: 'inst-page-text' },
            h('div', { class: 'target-pick' },
                h('button', { type: 'button', class: 'inst-page-name', title: '切換實例', 'aria-haspopup': 'menu', 'aria-expanded': 'false', onclick: () => toggleTargetMenu() },
                    h('h1', {}, instance ? instance.name : '尚未選擇實例'), icon('chevron-down')),
                h('div', { class: 'account-menu target-menu glass', role: 'menu', hidden: true })),
            instance
                ? h('div', { class: 'inst-page-meta' },
                    h('span', {}, icon('zap'), `${loaderLabel(instance, true)}・Minecraft ${instance.mcVersion}`),
                    h('span', {}, icon('clock'), fmtDuration(instance.playSeconds)),
                    instance.lastPlayed ? h('span', {}, `上次遊玩 ${new Date(instance.lastPlayed * 1000).toLocaleDateString('zh-TW')}`) : null)
                : h('div', { class: 'inst-page-meta' }, '請先到「實例」建立或選擇一個實例'),
            instance?.note ? h('div', { class: 'inst-note', title: instance.note }, icon('edit'), instance.note) : null),
        instance
            ? h('div', { class: 'inst-page-actions' },
                h('button', { type: 'button', class: `mc ${running ? 'danger' : 'primary'}`, onclick: () => playInstance(instance) }, icon(running ? 'close' : 'play'), running ? '結束遊戲' : '開始遊戲'),
                h('button', { type: 'button', class: 'mc icon-btn', title: '實例設定', 'aria-label': '實例設定', onclick: () => editInstance(instance) }, icon('edit')),
                moreMenu(instance))
            : null);
    for (const button of $$('#kind-tabs button')) button.setAttribute('aria-selected', button.dataset.kind === state.kind ? 'true' : 'false');
    for (const button of $$('.store-tabs button')) button.setAttribute('aria-selected', button.dataset.tab === state.tab ? 'true' : 'false');
    $('#store-explore').hidden = state.tab !== 'explore';
    $('#btn-mod-snapshots').hidden = state.kind !== 'mod';
    $('#store-installed').hidden = state.tab !== 'installed';
    $('#mod-search').placeholder = `搜尋 ${SOURCE_NAME[state.source]} 上的${KIND_LABEL[state.kind]}⋯⋯`;
    $('#store-source').value = state.source;
    $('#installed-source').value = state.source;
    const other = state.source === 'curseforge' ? 'Modrinth' : 'CurseForge';
    $('#installed-source-hint').textContent = `辨識、版本清單、更新與前置檢查先查 ${SOURCE_NAME[state.source]}，找不到的再查 ${other}；瀏覽內容也會預設用這個來源。`;
    $('#store-side').hidden = state.kind !== 'mod' || state.source !== 'modrinth';
    $('#installed-search').placeholder = `搜尋已安裝的${KIND_LABEL[state.kind]}⋯`;
}

export function openStore(kind, tab = state.tab) {
    if (!KIND_LABEL[kind]) return;
    state.tab = tab;
    if (state.kind === kind) return renderAll();
    state.kind = kind;
    state.category = '';
    state.filter = '';
    $('#installed-search').value = '';
    renderCategories();
    renderAll();
    reloadInstalled();
    search();
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
    const kinds = state.kind === 'mod' ? ['mod', 'shader'] : [state.kind, 'mod'];
    await Promise.all(kinds.map(loadInstalled));
    if (seq === state.listSeq) renderAll();
}

async function setSource(source) {
    const instance = selectedInstance();
    if (!SOURCE_NAME[source] || source === state.source) return;
    state.source = source;
    state.category = '';
    renderCategories();
    if (instance) {
        instance.contentSource = source;
        await api.set_content_source({ id: instance.id, source }).catch(fail);
        onInstanceChange.last = `${instance.id}|${instance.mcVersion}|${instance.loader}|${source}`;
    }
    installed.mod = installed.resourcepack = installed.shader = null;
    renderAll();
    reloadInstalled();
    search();
}

function onInstanceChange() {
    if (location.hash !== '#mods') return;
    const instance = selectedInstance();
    const source = instance?.contentSource === 'curseforge' ? 'curseforge' : 'modrinth';
    const key = `${instance?.id}|${instance?.mcVersion}|${instance?.loader}|${source}`;
    if (key === onInstanceChange.last) return;
    onInstanceChange.last = key;
    if (state.source !== source) {
        state.source = source;
        state.category = '';
        renderCategories();
    }
    installed.mod = installed.resourcepack = installed.shader = null;
    renderAll();
    reloadInstalled();
    search();
}

async function warnRisky(instance, files) {
    if (instance.general || !files?.length) return;
    const risky = await api.mod_risks({ id: instance.id, files }).catch(() => []);
    if (!risky.length) return;
    const remove = await dialog({
        title: '這個模組可能違反輝煌服規',
        content: h('div', {},
            h('ul', {}, ...risky.map((m) => h('li', {}, h('b', {}, m.name), `：${m.reason}`))),
            h('p', {}, '輝煌服規 1-1：不允許修改遊戲數據或嚴重影響遊戲公平性的行為（例如加快挖礦、取消後搖、進食更快），違者會被停權。自動挖礦、釣魚、蓋房子這類自動腳本不受影響。'),
            h('p', { class: 'hint' }, '只在單人遊戲或其他伺服器使用的話，可以保留；也可以改用「一般實例」安裝，就不會再提醒。')),
        buttons: [{ label: '我了解，保留', value: null }, { label: '移除這些模組', value: 'remove' }]
    });
    if (remove !== 'remove') return;
    for (const m of risky) await api.content_remove({ id: instance.id, kind: 'mod', file: m.file }).catch(fail);
    toast(`已移除 ${risky.length} 個模組`);
    await loadInstalled('mod');
    renderAll();
}

const KIND_NAME = { mod: '模組', resourcepack: '資源包', shader: '光影' };

async function installLocal(paths, instance = selectedInstance()) {
    if (!instance) return toast('請先選擇一個實例', 'error');
    const files = paths.filter((p) => isType(p, 'content'));
    if (!files.length) return toast(`這裡只能放${typeLabel('content')}`, 'warn');
    try {
        const added = await api.add_content({ id: instance.id, files });
        const kinds = [...new Set(added.map(([kind]) => kind))];
        toast(`已安裝 ${kinds.map((kind) => `${added.filter(([k]) => k === kind).length} 個${KIND_NAME[kind]}`).join('、')}到「${instance.name}」`);
        if (instance.id === selectedInstance()?.id) {
            await Promise.all(kinds.map(loadInstalled));
            renderAll();
        }
        await warnRisky(instance, added.filter(([kind]) => kind === 'mod').map(([, file]) => file));
    } catch (error) {
        fail(error);
    }
}

function dropCard(payload) {
    const at = payload?.position;
    if (!at || !$('#view-instances').classList.contains('active')) return null;
    const card = document.elementFromPoint(at.x / devicePixelRatio, at.y / devicePixelRatio)?.closest('.inst-card[data-id]');
    return card ? { card, instance: store.instances.find((i) => i.id === card.dataset.id) } : null;
}

function markDropCard(card) {
    for (const other of $$('.inst-card.drop-hover')) if (other !== card) other.classList.remove('drop-hover');
    card?.classList.add('drop-hover');
}

function zoneFor(payload) {
    const at = payload?.position;
    return at ? fileZoneAt(at.x / devicePixelRatio, at.y / devicePixelRatio) : null;
}

function markZone(el) {
    for (const other of $$('.file-drop.drop-active')) if (other !== el) other.classList.remove('drop-active');
    el?.classList.add('drop-active');
}

function dropHint(payload) {
    let hint = $('#drop-hint');
    if (!payload) {
        if (hint) hint.hidden = true;
        markDropCard(null);
        markZone(null);
        return;
    }
    if (!hint) {
        hint = h('div', { id: 'drop-hint', class: 'drop-hint', role: 'status' });
        document.body.append(hint);
    }
    hint.hidden = false;
    const zone = zoneFor(payload);
    markZone(zone?.el);
    if (zone || modalOpen()) {
        markDropCard(null);
        hint.textContent = zone ? `放開即可加入${typeLabel(zone.type)}` : '請拖到視窗中對應的欄位上';
        return;
    }
    const hit = dropCard(payload);
    markDropCard(hit?.card);
    const onMods = $('#view-mods').classList.contains('active');
    const target = onMods ? selectedInstance() : hit?.instance ?? (store.instances.length === 1 ? store.instances[0] : null);
    hint.textContent = target ? `放開即可把模組、資源包或光影安裝到「${target.name}」`
        : store.instances.length ? '放開後選擇要安裝到哪個實例（在「實例」頁可直接拖到卡片上）' : '請先建立一個實例';
    hint.hidden = false;
}

function pickDropTarget(paths) {
    const names = paths.filter((p) => isType(p, 'content')).map((p) => p.split(/[\\/]/).pop());
    const current = selectedInstance();
    return dialog({
        title: '要安裝到哪個實例？',
        wide: true,
        content: h('div', {},
            h('p', { class: 'hint' }, `${names.slice(0, 3).join('、')}${names.length > 3 ? ` 等 ${names.length} 個檔案` : ''}，會依內容自動放進模組、資源包或光影資料夾。`),
            h('div', { class: 'drop-targets' }, ...store.instances.map((instance) => h('button', {
                type: 'button',
                class: `drop-target${instance.id === current?.id ? ' current' : ''}`,
                onclick: () => dismissDialog(instance)
            }, instanceVisual(instance), h('b', {}, instance.name), h('small', {}, `${instance.mcVersion}・${loaderLabel(instance)}`))))),
        buttons: [{ label: '取消', value: null }]
    });
}

export function initMods() {
    for (const button of $$('#kind-tabs button')) {
        button.addEventListener('click', () => {
            state.kind = button.dataset.kind;
            state.category = '';
            state.filter = '';
            $('#installed-search').value = '';
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
    $('#store-side').addEventListener('change', () => search());
    $('#store-source').addEventListener('change', (e) => setSource(e.target.value));
    $('#installed-source').addEventListener('change', (e) => setSource(e.target.value));
    $('#btn-more-mods').addEventListener('click', guard(() => search(false)));
    new IntersectionObserver((entries) => {
        if (entries.some((e) => e.isIntersecting) && !$('#btn-more-mods').hidden && !state.loadingMore) search(false);
    }, { rootMargin: '300px' }).observe($('#btn-more-mods'));
    $('#btn-refresh-mods').addEventListener('click', guard(() => reloadInstalled()));
    $('#btn-inst-back').addEventListener('click', () => { location.hash = '#instances'; });
    $('#installed-search').addEventListener('input', (e) => {
        state.filter = e.target.value;
        renderInstalled();
    });
    $('#btn-update-all').addEventListener('click', guard(() => update(state.kind, (installed[state.kind] || []).filter((entry) => entry.update && !entry.pinned))));
    $('#btn-add-mod').addEventListener('click', guard(async () => {
        const picked = await pickFile('content', { multiple: true });
        if (picked) await installLocal(Array.isArray(picked) ? picked : [picked]);
    }));
    $('#btn-open-mods').addEventListener('click', () => openFolder({ mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks' }[state.kind]));
    $('#btn-mod-snapshots').addEventListener('click', guard(() => modSnapshots().catch(fail)));
    for (const button of $$('[data-folder]')) button.addEventListener('click', () => openFolder(button.dataset.folder));

    listen('tauri://drag-enter', (payload) => dropHint(payload ?? {}));
    listen('tauri://drag-over', (payload) => dropHint(payload ?? {}));
    listen('tauri://drag-leave', () => dropHint(null));
    listen('tauri://drag-drop', async (payload) => {
        const zone = zoneFor(payload);
        const hit = dropCard(payload)?.instance;
        dropHint(null);
        const paths = payload?.paths ?? [];
        if (zone) {
            const ok = paths.filter((p) => isType(p, zone.type));
            if (!ok.length) return toast(`這裡只能放${typeLabel(zone.type)}`, 'warn');
            return zone.handler(zone.multiple ? ok : ok[0]);
        }
        if (modalOpen()) return toast('視窗開著時，請把檔案拖到視窗中對應的欄位上', 'warn');
        const pack = paths.find((p) => isType(p, 'mrpack'));
        if (pack) return importPack($('#view-mods').classList.contains('active') ? selectedInstance()?.id ?? null : null, pack);
        if ($('#view-mods').classList.contains('active')) return installLocal(paths);
        if (!paths.some((p) => isType(p, 'content'))) return installLocal(paths);
        const target = hit ?? (store.instances.length === 1 ? store.instances[0] : await pickDropTarget(paths));
        if (target) installLocal(paths, target);
    });

    on('instances', onInstanceChange);
    on('instances', () => { if (location.hash === '#mods') renderHeader(); });
    on('settings', onInstanceChange);
    addEventListener('hashchange', onInstanceChange);
    watchFolders(['mods', 'resourcepacks', 'shaderpacks'], () => (location.hash === '#mods' ? selectedInstance()?.id : undefined), reloadInstalled);
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
