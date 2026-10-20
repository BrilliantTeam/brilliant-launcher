import { api, fileSrc, openUrl, pickFiles } from '../api.js';
import { $, clear, confirm, dialog, fail, fmtBytes, fmtDuration, h, rangeFill, toast, guard } from '../ui.js';
import { emit, isRunning, loadInstances, on, selectedInstance, store } from '../store.js';
import { icon, INSTANCE_ICONS, instanceIconName } from '../icons.js';
import { importPack } from './mods.js';
import { isPreparing, repairInstance } from './home.js';
import { jvmPresetButtons } from './settings.js';
import { perfCheck } from '../perf.js';

export async function versionData() {
    if (!store.versions) store.versions = await api.list_versions();
    return store.versions;
}

function segmented(options, value) {
    const box = h('div', { class: 'segmented', role: 'radiogroup' });
    box.value = value;
    const buttons = options.map(([v, label]) =>
        h('button', {
            type: 'button',
            role: 'radio',
            'aria-selected': String(v === value),
            'aria-checked': String(v === value),
            onclick: () => {
                box.value = v;
                buttons.forEach((b, i) => {
                    b.setAttribute('aria-selected', String(options[i][0] === v));
                    b.setAttribute('aria-checked', String(options[i][0] === v));
                });
                box.dispatchEvent(new Event('change'));
            }
        }, label)
    );
    box.append(...buttons);
    return box;
}

async function editorDialog(title, source) {
    const data = await versionData();
    const modSupported = new Set(data.modVersions);

    const name = h('input', { type: 'text', value: source.name ?? '', maxlength: '40' });
    let chosenIcon = instanceIconName(source.icon);
    const iconPicker = h('div', { class: 'icon-picker' });
    const iconButtons = INSTANCE_ICONS.map((name) =>
        h('button', {
            type: 'button',
            class: 'mc',
            title: name,
            'aria-pressed': name === chosenIcon ? 'true' : 'false',
            onclick: () => {
                chosenIcon = name;
                iconButtons.forEach((button, i) => button.setAttribute('aria-pressed', INSTANCE_ICONS[i] === name ? 'true' : 'false'));
            }
        }, icon(name))
    );
    iconPicker.append(...iconButtons);
    let pickedImage;
    const preview = h('div', { class: 'image-preview' });
    const imageClear = h('button', { type: 'button', class: 'mc', onclick: () => { pickedImage = ''; renderPreview(); } }, '移除圖片');
    const renderPreview = () => {
        const current = pickedImage === undefined ? source.image : pickedImage;
        preview.style.backgroundImage = current ? `url("${fileSrc(current)}")` : '';
        preview.classList.toggle('empty', !current);
        imageClear.disabled = !current;
    };
    const imageField = h('div', { class: 'field' }, h('label', {}, '封面圖片（選填，會取代圖示）'),
        h('div', { class: 'image-pick' }, preview,
            h('div', { class: 'toolbar' },
                h('button', {
                    type: 'button',
                    class: 'mc',
                    onclick: async () => {
                        const path = await pickFiles({ multiple: false, filters: [{ name: '圖片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }] }).catch(fail);
                        if (path) { pickedImage = path; renderPreview(); }
                    }
                }, icon('image'), '選擇圖片'),
                imageClear)));
    renderPreview();
    const wanted = source.mcVersion || (source.general ? data.latestRelease : data.recommended) || '';
    let showAll = !!source.general || !modSupported.size || (wanted !== '' && !modSupported.has(wanted));
    const kind = segmented([['brilliant', '輝煌實例'], ['general', '一般實例']], source.general ? 'general' : 'brilliant');
    const kindHint = h('div', { class: 'hint' });
    const mcVersion = h('select', {});
    const renderVersions = () => {
        const current = mcVersion.value || wanted;
        clear(mcVersion).append(
            ...data.versions
                .filter((v) => showAll || modSupported.has(v.id))
                .map((v) =>
                    h(
                        'option',
                        { value: v.id, selected: v.id === current },
                        `${v.id}${v.type === 'release' ? '' : `（${v.type}）`}${v.id === data.recommended ? (kind.value === 'general' ? '（輝煌推薦）' : '（推薦）') : showAll && modSupported.has(v.id) ? '（輝煌支援）' : ''}`
                    )
                )
        );
    };
    renderVersions();
    const showAllBox = h('label', { style: 'margin-top:8px' },
        h('input', { type: 'checkbox', checked: showAll, onchange: (e) => { showAll = e.target.checked; renderVersions(); refreshLoaders(); } }),
        '顯示所有 Minecraft 版本（輝煌不支援的版本無法連上伺服器）');
    const loader = segmented([['fabric', 'Fabric'], ['vanilla', '原版']], source.loader === 'fabric' ? 'fabric' : 'vanilla');
    const loaderVersion = h('select', {}, h('option', { value: '' }, '最新穩定版'),
        source.loaderVersion ? h('option', { value: source.loaderVersion, selected: true }, source.loaderVersion) : '');
    let loaderSeq = 0;
    const memoryLabel = h('b', {});
    const memory = h('input', { type: 'range', min: '0', max: '32768', step: '512', value: String(source.memoryMb ?? 0) });
    const syncMemory = () => {
        memoryLabel.textContent = Number(memory.value) ? `${memory.value} MB` : `沿用全域設定（${store.settings?.memoryMb ?? 4096} MB）`;
        rangeFill(memory);
    };
    memory.addEventListener('input', syncMemory);
    syncMemory();
    const rpc = segmented([['', '沿用全域'], ['on', '啟用'], ['off', '停用']],
        source.injectRichPresence === true ? 'on' : source.injectRichPresence === false ? 'off' : '');

    const loaderField = h('div', { class: 'field' }, h('label', {}, 'Fabric Loader 版本'), loaderVersion);
    const rpcField = h('div', { class: 'field' }, h('label', {}, '自動安裝 Brilliant-Server-Mod'), rpc,
        h('div', { class: 'hint' }, '啟用時，每次啟動都會自動安裝最新的 Brilliant-Server-Mod 與 Fabric API。'));

    async function refreshLoaders() {
        const isFabric = loader.value === 'fabric';
        const general = kind.value === 'general';
        loaderField.style.display = isFabric ? '' : 'none';
        rpcField.style.display = isFabric && !general ? '' : 'none';
        showAllBox.style.display = general ? 'none' : '';
        kindHint.textContent = general
            ? '給合作伺服器或單人遊戲用：任何版本都能選，不會自動安裝輝煌模組、不會預載伺服器資源包，也不會跳出輝煌的升版提醒。'
            : '玩輝煌用：自動安裝 Brilliant-Server-Mod、預載伺服器資源包，伺服器升版時會提醒你一鍵跟上。';
        if (!isFabric) return;
        const seq = ++loaderSeq;
        const keep = loaderVersion.value;
        loaderVersion.disabled = true;
        try {
            const list = await api.list_fabric_loaders({ gameVersion: mcVersion.value });
            if (seq !== loaderSeq) return;
            clear(loaderVersion).append(
                h('option', { value: '' }, '最新穩定版'),
                ...list.map((l) =>
                    h('option', { value: l.version, selected: l.version === keep },
                        `${l.version}${l.stable ? '' : '（測試版）'}`)
                )
            );
        } catch (error) {
            if (seq === loaderSeq) fail(error);
        } finally {
            if (seq === loaderSeq) loaderVersion.disabled = false;
        }
    }
    loader.addEventListener('change', refreshLoaders);
    kind.addEventListener('change', () => {
        showAll = kind.value === 'general' || !modSupported.size;
        showAllBox.querySelector('input').checked = showAll;
        renderVersions();
        refreshLoaders();
    });
    mcVersion.addEventListener('change', refreshLoaders);

    const javaPath = h('select', {}, h('option', { value: '' }, '沿用全域設定'),
        source.javaPath ? h('option', { value: source.javaPath, selected: true }, source.javaPath) : '');
    api.list_java().then((list) => {
        const keep = javaPath.value;
        clear(javaPath).append(h('option', { value: '' }, '沿用全域設定'),
            ...list.map((j) => h('option', { value: j.path, selected: j.path === keep }, `Java ${j.version} — ${j.path}`)));
        if (keep && !list.some((j) => j.path === keep)) javaPath.append(h('option', { value: keep, selected: true }, keep));
    }).catch(() => {});
    const jvmArgs = h('textarea', { spellcheck: 'false', placeholder: '留空沿用全域設定' }, source.jvmArgs ?? '');
    const width = h('input', { type: 'number', min: '0', max: '7680', value: source.width || '', placeholder: '沿用全域' });
    const height = h('input', { type: 'number', min: '0', max: '4320', value: source.height || '', placeholder: '沿用全域' });
    const fullscreen = h('input', { type: 'checkbox', checked: !!source.fullscreen });
    const javaMajor = h('select', {},
        ...[[0, '自動（依 Minecraft 版本）'], [8, 'Java 8'], [17, 'Java 17'], [21, 'Java 21'], [25, 'Java 25']].map(([value, label]) =>
            h('option', { value: String(value), selected: Number(source.javaMajor || 0) === value }, label)));
    const wrapper = h('input', { type: 'text', spellcheck: 'false', value: source.wrapper ?? '', placeholder: '留空沿用全域設定' });
    const envVars = h('textarea', { spellcheck: 'false', placeholder: '每行一個 KEY=VALUE，會覆蓋全域的同名變數' }, source.envVars ?? '');
    const useShared = h('input', { type: 'checkbox', checked: !!source.useShared });
    const sharedList = (store.settings?.sharedFolders ?? []).map((f) => ({ saves: '存檔', resourcepacks: '資源包', shaderpacks: '光影', screenshots: '截圖' })[f]).filter(Boolean);
    const modChannel = h('select', {},
        ...[['', '沿用全域設定'], ['release', '只用正式版'], ['beta', '允許 Beta 版'], ['alpha', '允許所有版本']].map(([value, label]) =>
            h('option', { value, selected: (source.modChannel ?? '') === value }, label)));
    const freezeMods = h('input', { type: 'checkbox', checked: !!source.freezeMods });
    const advanced = h('details', { class: 'advanced', open: !!(source.javaPath || source.jvmArgs || source.width || source.javaMajor || source.wrapper || source.envVars || source.useShared || source.modChannel || source.freezeMods) },
        h('summary', {}, '進階設定（覆寫全域）'),
        h('div', { class: 'field-row' },
            h('div', { class: 'field' }, h('label', {}, 'Java 版本'), javaMajor),
            h('div', { class: 'field' }, h('label', {}, 'Java 執行檔'), javaPath)),
        h('div', { class: 'hint' }, '指定版本時，啟動器會自動下載並管理對應的 Java；選了「Java 執行檔」則以執行檔為準。'),
        h('div', { class: 'field' }, h('label', {}, 'JVM 參數'), jvmArgs, h('div', { class: 'jvm-presets' }, ...jvmPresetButtons(jvmArgs, () => {}))),
        h('div', { class: 'field' }, h('label', {}, '啟動包裝指令'), wrapper),
        h('div', { class: 'field' }, h('label', {}, '環境變數'), envVars),
        h('div', { class: 'field' },
            h('label', {}, useShared, '使用共用資料夾'),
            h('div', { class: 'hint' }, sharedList.length ? `會共用：${sharedList.join('、')}（在「設定 → 進階」調整）` : '目前沒有選擇要共用的資料夾，請先到「設定 → 進階」勾選。')),
        h('div', { class: 'field-row' },
            h('div', { class: 'field' }, h('label', {}, '模組更新管道'), modChannel),
            h('div', { class: 'field' }, h('label', {}, freezeMods, '凍結模組'), h('div', { class: 'hint' }, '不檢查也不自動更新這個實例的模組，適合穩定的整合包。'))),
        h('div', { class: 'field-row' },
            h('div', { class: 'field' }, h('label', {}, '視窗寬度'), width),
            h('div', { class: 'field' }, h('label', {}, '視窗高度'), height)),
        h('label', {}, fullscreen, '全螢幕啟動（需同時填寫寬高才會覆寫）'));

    const content = h(
        'div',
        { class: 'inst-editor' },
        h('div', { class: 'field' }, h('label', {}, '類型'), kind, kindHint),
        h('div', { class: 'field' }, h('label', {}, '名稱'), name),
        h('div', { class: 'field' }, h('label', {}, '圖示'), iconPicker),
        imageField,
        h('div', { class: 'field' }, h('label', {}, 'Minecraft 版本'), mcVersion, modSupported.size ? showAllBox : ''),
        h('div', { class: 'field' }, h('label', {}, '模組載入器'), loader),
        loaderField,
        h('div', { class: 'field' }, h('label', {}, '記憶體上限：', memoryLabel), memory),
        rpcField,
        advanced
    );

    refreshLoaders();

    const ok = await dialog({
        title,
        content,
        buttons: [
            { label: '取消', value: false },
            { label: '確定', value: true }
        ]
    });
    if (ok !== true) return null;
    if (!name.value.trim()) {
        toast('請輸入實例名稱', 'error');
        return null;
    }
    return {
        ...source,
        name: name.value.trim(),
        icon: chosenIcon,
        mcVersion: mcVersion.value,
        loader: loader.value,
        loaderVersion: loader.value === 'fabric' ? loaderVersion.value : '',
        memoryMb: Math.max(0, Number(memory.value) || 0),
        injectRichPresence: rpc.value === '' ? null : rpc.value === 'on',
        general: kind.value === 'general',
        javaPath: javaPath.value,
        jvmArgs: jvmArgs.value.trim(),
        width: Math.max(0, Number(width.value) || 0),
        height: Math.max(0, Number(height.value) || 0),
        fullscreen: fullscreen.checked,
        javaMajor: Number(javaMajor.value) || 0,
        wrapper: wrapper.value.trim(),
        envVars: envVars.value.trim(),
        useShared: useShared.checked,
        modChannel: modChannel.value,
        freezeMods: freezeMods.checked,
        pickedImage
    };
}

const BLANK = {
    id: '', name: '', icon: 'zap', mcVersion: '', loader: 'vanilla', loaderVersion: '',
    versionId: '', memoryMb: 0, jvmArgs: '', width: 0, height: 0, fullscreen: false,
    injectRichPresence: null, general: false, join: '', createdAt: 0, lastPlayed: 0, playSeconds: 0,
    javaMajor: 0, wrapper: '', envVars: '', useShared: false, modChannel: '', freezeMods: false
};

async function applyImage(id, pickedImage) {
    if (pickedImage === undefined) return;
    await api.set_instance_image({ id, path: pickedImage }).catch(fail);
}

async function createFromDraft({ pickedImage, ...draft }) {
    toast('正在準備版本資訊⋯⋯');
    const created = await api.create_instance({ draft });
    await applyImage(created.id, pickedImage);
    await api.select_instance({ id: created.id });
    store.settings.selectedInstance = created.id;
    await loadInstances();
    emit('settings');
    toast(`已建立「${created.name}」`);
    return created;
}

export async function quickCreate() {
    const data = await versionData();
    const version = data.recommended || data.modVersions[0];
    if (!version) throw new Error('暫時連不上官網，無法取得輝煌支援的版本，請稍後再試，或到「實例」頁手動建立。');
    return createFromDraft({ ...BLANK, name: `輝煌 ${version}`, icon: 'zap', mcVersion: version, loader: 'fabric' });
}

async function createInstance(general = false) {
    const draft = await editorDialog('建立實例', { ...BLANK, loader: 'fabric', general });
    if (!draft) return;
    await createFromDraft(draft).catch(fail);
}

export const createGeneralInstance = () => createInstance(true);

async function editInstance(instance) {
    const result = await editorDialog('編輯實例', { ...instance });
    if (!result) return;
    const { pickedImage, ...draft } = result;
    try {
        await api.update_instance({ updated: draft });
        await applyImage(draft.id, pickedImage);
        await loadInstances();
        toast('已儲存');
    } catch (error) {
        fail(error);
    }
}

async function duplicateInstance(instance) {
    const input = h('input', { type: 'text', value: `${instance.name} 的複本`, maxlength: '40' });
    const ok = await dialog({
        title: '複製實例',
        content: h('div', { class: 'field' },
            h('label', {}, '新實例名稱'), input,
            h('div', { class: 'hint' }, '存檔、設定與模組都會一併複製。')),
        buttons: [{ label: '取消', value: false }, { label: '複製', value: true }]
    });
    if (ok !== true) return;
    try {
        await api.duplicate_instance({ id: instance.id, name: input.value.trim() });
        await loadInstances();
        toast('已複製');
    } catch (error) {
        fail(error);
    }
}

async function removeInstance(instance) {
    if (!(await confirm(`確定要刪除「${instance.name}」嗎？\n這個實例的存檔與模組都會一起消失，無法復原。`, '刪除'))) return;
    try {
        await api.delete_instance({ id: instance.id });
        await loadInstances();
        toast('已刪除');
    } catch (error) {
        fail(error);
    }
}

async function openFolder(instance, sub = '') {
    try {
        const path = await api.open_instance_folder({ id: instance.id, sub });
        await api.open_path({ path });
    } catch (error) {
        fail(error);
    }
}

export async function selectInstance(instance) {
    if (selectedInstance()?.id === instance.id) return;
    await api.select_instance({ id: instance.id });
    store.settings.selectedInstance = instance.id;
    emit('instances');
    emit('settings');
}

const PLAN_LABEL = {
    managed: ['自動處理', ''],
    update: ['會更新', 'accent'],
    keep: ['可直接使用', ''],
    disable: ['新版尚未支援，先停用', 'warn'],
    unknown: ['無法確認，保留不動', 'warn']
};

function planOf(entry) {
    if (entry.managed) return 'managed';
    if (!entry.enabled) return 'keep';
    if (entry.update) return 'update';
    if (entry.compatible === true) return 'keep';
    if (entry.compatible === false) return 'disable';
    return 'unknown';
}

export async function upgradeFlow(instance, target) {
    const list = h('div', { class: 'rows upgrade-list' }, h('div', { class: 'empty-state' }, '正在比對模組相容性⋯'));
    const mode = segmented([['inplace', '直接升級（先自動備份存檔）'], ['copy', '複製一份再升級']], 'inplace');
    let plan = [];
    api.content_list({ id: instance.id, kind: 'mod', mc: target }).then((entries) => {
        plan = entries.map((entry) => ({ entry, action: planOf(entry) }));
        const count = (a) => plan.filter((p) => p.action === a).length;
        const summary = [['update', '個會更新'], ['keep', '個可直接使用'], ['disable', '個新版尚未支援（先停用）'], ['unknown', '個無法確認']]
            .filter(([a]) => count(a)).map(([a, label]) => `${count(a)} ${label}`).join('、');
        clear(list).append(...(summary ? [h('div', { class: 'hint' }, summary)] : []), ...(plan.length ? plan.map(({ entry, action }) => h('div', { class: 'row' },
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, entry.name),
                h('div', { class: 'meta' }, action === 'update' ? `${entry.version ?? ''} → ${entry.update.version}` : entry.file)),
            h('span', { class: `tag ${PLAN_LABEL[action][1]}` }, PLAN_LABEL[action][0])))
            : [h('div', { class: 'empty-state' }, '這個實例沒有安裝其他模組。')]));
    }).catch((error) => {
        clear(list).append(h('div', { class: 'empty-state' }, '暫時無法連上 Modrinth，模組會保持原樣，升級後可到「模組與資源」檢查更新。'));
        fail(error);
    });
    const go = await dialog({
        title: `升級到 Minecraft ${target}`,
        wide: true,
        content: h('div', {},
            h('p', { class: 'hint' }, instance.loader === 'fabric'
                ? `「${instance.name}」目前是 ${instance.mcVersion}。升級後 Fabric Loader 會換成最新穩定版，Brilliant-Server-Mod 與 Fabric API 會在下次啟動自動換成對應版本。`
                : `「${instance.name}」目前是 ${instance.mcVersion}，升級後下次開始遊戲會自動下載新版本。`),
            !instance.general && store.versions?.modVersions?.length && !store.versions.modVersions.includes(target)
                ? h('p', { class: 'hint warn' }, `輝煌伺服器目前還不支援 ${target}，升級後可能無法連線輝煌。`)
                : '',
            mode,
            list),
        buttons: [{ label: '取消', value: false }, { label: '開始升級', value: true }]
    });
    if (go !== true) return;
    try {
        let targetInstance = instance;
        if (mode.value === 'copy') {
            toast('正在複製實例⋯');
            targetInstance = await api.duplicate_instance({ id: instance.id, name: `${instance.name}（${target}）` });
        } else {
            await api.backup_now({ id: instance.id }).catch(() => {});
        }
        toast(`正在升級到 ${target}⋯`);
        await api.update_instance({ updated: { ...targetInstance, mcVersion: target, loaderVersion: '' } });
        for (const { entry, action } of plan) {
            if (action === 'update') await api.content_update({ id: targetInstance.id, kind: 'mod', file: entry.file, update: entry.update }).catch(fail);
            if (action === 'disable') await api.set_mod_enabled({ id: targetInstance.id, file: entry.file, enabled: false }).catch(fail);
        }
        const dropped = new Set(plan.filter((p) => p.action === 'disable').map(({ entry }) => entry.project_id || entry.file.replace(/\.disabled$/, '')));
        if (dropped.size && targetInstance.modProfiles?.length) {
            const profiles = targetInstance.modProfiles.map((p) => ({ ...p, enabled: p.enabled.filter((key) => !dropped.has(key)) }));
            await api.set_mod_profiles({ id: targetInstance.id, profiles, active: targetInstance.activeProfile || '' }).catch(fail);
        }
        await api.select_instance({ id: targetInstance.id });
        store.settings.selectedInstance = targetInstance.id;
        await loadInstances();
        emit('settings');
        toast(`已升級到 Minecraft ${target}`);
        api.prefetch({ id: targetInstance.id }).catch(() => {});
    } catch (error) {
        fail(error);
    }
}

async function pickUpgrade(instance) {
    const data = await versionData().catch(() => null);
    if (!data) return toast('暫時無法取得版本清單', 'warn');
    const index = data.versions.findIndex((v) => v.id === instance.mcVersion);
    const newer = data.versions.slice(0, index < 0 ? 0 : index).filter((v) => v.type === 'release');
    if (!newer.length) return toast(`「${instance.name}」已經是最新的正式版 ${instance.mcVersion}`);
    const supported = new Set(data.modVersions);
    const select = h('select', {}, ...newer.map((v) => h('option', { value: v.id, selected: v.id === data.recommended },
        `${v.id}${v.id === data.recommended ? '（輝煌推薦）' : supported.has(v.id) ? '（輝煌支援）' : ''}`)));
    const go = await dialog({
        title: `升級「${instance.name}」`,
        content: h('div', { class: 'field' },
            h('label', {}, `從 ${instance.mcVersion} 升級到`),
            select,
            h('div', { class: 'hint' }, '下一步會逐一檢查每個模組在新版本有沒有對應版本，再決定更新、保留或先停用。')),
        buttons: [{ label: '取消', value: false }, { label: '檢查相容性', value: true }]
    });
    if (go === true) upgradeFlow(instance, select.value);
}

const PART_LABEL = {
    saves: '存檔',
    'options.txt': '遊戲設定與按鍵',
    'servers.dat': '伺服器清單',
    resourcepacks: '資源包',
    shaderpacks: '光影',
    screenshots: '截圖',
    mods: '模組（版本相同時）',
    config: '模組設定檔'
};

export async function importFlow() {
    const box = h('div', { class: 'rows import-list' }, h('div', { class: 'empty-state' }, '正在尋找電腦上的其他啟動器⋯'));
    let chosen = null;
    const LOADER = { fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' };
    const show = (sources, empty) => clear(box).append(...(sources.length ? sources.map((src) => h('button', {
        type: 'button',
        class: 'mc import-source',
        onclick: () => { chosen = src; $('#dlg-foot button').click(); }
    },
    h('span', { class: 'grow' },
        h('b', {}, src.name),
        h('small', {}, `${src.launcher}｜${src.mcVersion || '版本未知'}${LOADER[src.loader] ? `・${LOADER[src.loader]} ${src.loaderVersion}` : ''}｜存檔 ${src.saves}・模組 ${src.mods}`)),
    icon('arrow-right')))
        : [h('div', { class: 'empty-state' }, empty)]));
    const pickFolder = h('button', {
        type: 'button',
        class: 'mc',
        onclick: async () => {
            const path = await pickFiles({ directory: true, multiple: false }).catch(fail);
            if (!path) return;
            show(await api.import_inspect({ path }).catch((error) => (fail(error), [])), '這個資料夾裡找不到可以匯入的實例。可以選啟動器的資料夾、instances 資料夾，或單一實例資料夾。');
        }
    }, icon('folder'), '手動選擇資料夾');
    const data = await versionData().catch(() => null);
    api.import_sources().then((sources) => show(sources, '沒有自動找到其他啟動器。MultiMC 這類免安裝版可以用「手動選擇資料夾」。')).catch(fail);
    await dialog({
        title: '從其他啟動器匯入',
        wide: true,
        content: h('div', {},
            h('div', { class: 'toolbar' },
                h('span', { class: 'toolbar-label' }, '支援官方啟動器、Prism、MultiMC、PolyMC、CurseForge、Modrinth App、X Minecraft Launcher。原本的資料不會被移動或刪除。'),
                h('span', { class: 'spacer' }),
                h('button', { type: 'button', class: 'mc', onclick: () => { $('#dlg').close(); importPack(null); } }, icon('briefcase'), '匯入 .mrpack 模組包'),
                pickFolder),
            box),
        buttons: [{ label: '取消', value: null }]
    });
    if (!chosen) return;

    const known = data?.versions?.some((v) => v.id === chosen.mcVersion);
    const mc = known ? chosen.mcVersion : (data?.recommended || '');
    const name = h('input', { type: 'text', value: chosen.name.slice(0, 40), maxlength: '40' });
    const target = segmented([['new', '建立新實例'], ['current', '匯入到目前的實例']], 'new');
    const checks = Object.entries(PART_LABEL).map(([part, label]) => {
        const sameGame = chosen.mcVersion === mc && chosen.loader === 'fabric';
        const input = h('input', { type: 'checkbox', checked: part === 'mods' ? sameGame : part !== 'screenshots', value: part });
        return h('label', {}, input, label);
    });
    const ok = await dialog({
        title: `匯入「${chosen.name}」`,
        content: h('div', {},
            h('div', { class: 'field' }, target),
            h('div', { class: 'field' }, h('label', {}, '新實例名稱'), name,
                h('div', { class: 'hint' }, `會建立 Minecraft ${mc || '（請先選版本）'}${chosen.loader === 'fabric' ? ' + Fabric' : ''} 的實例${known ? '' : '（原版本不在清單上，改用推薦版本）'}。${['forge', 'neoforge', 'quilt'].includes(chosen.loader) ? `輝煌啟動器只支援 Fabric，${LOADER[chosen.loader]} 模組不會帶過來。` : ''}`)),
            h('div', { class: 'field import-parts' }, h('label', {}, '要帶過來的內容'), ...checks)),
        buttons: [{ label: '取消', value: false }, { label: '開始匯入', value: true }]
    });
    if (ok !== true) return;
    const parts = checks.map((label) => label.querySelector('input')).filter((i) => i.checked).map((i) => i.value);
    try {
        let instance = selectedInstance();
        if (target.value === 'new' || !instance) {
            if (!mc) throw new Error('無法判斷要用的 Minecraft 版本');
            instance = await createFromDraft({ ...BLANK, name: name.value.trim() || chosen.name, icon: 'archive', mcVersion: mc, loader: chosen.loader === 'fabric' ? 'fabric' : 'vanilla', loaderVersion: known ? chosen.loaderVersion : '' });
        }
        toast('正在複製檔案⋯');
        const bytes = await api.import_into({ request: { path: chosen.path, instanceId: instance.id, parts } });
        toast(`匯入完成，共 ${fmtBytes(bytes)}`);
    } catch (error) {
        fail(error);
    }
}

const GAME_MODE = ['生存', '創造', '冒險', '旁觀'];

async function showWorlds(instance) {
    const list = h('div', { class: 'rows world-list' }, h('div', { class: 'empty-state' }, '讀取世界清單⋯'));
    const refresh = async () => {
        const worlds = await api.list_worlds({ id: instance.id }).catch((error) => (fail(error), []));
        clear(list).append(...(worlds.length ? worlds.map((w) => h('div', { class: 'row world-row' },
            w.icon ? h('img', { class: 'world-icon', src: fileSrc(w.icon), alt: '' }) : h('span', { class: 'world-icon' }, icon('map')),
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, w.name, w.hardcore ? h('span', { class: 'badge warn' }, '極限') : null),
                h('div', { class: 'meta' }, [
                    GAME_MODE[w.gameMode] ?? '生存',
                    fmtBytes(w.size),
                    w.lastPlayed ? `上次遊玩 ${new Date(w.lastPlayed).toLocaleString('zh-TW', { hour12: false })}` : '',
                    w.folder !== w.name ? `資料夾：${w.folder}` : ''
                ].filter(Boolean).join('｜'))),
            h('button', { type: 'button', class: 'mc', title: '複製到其他實例', onclick: () => copyWorld(instance, w).then(refresh) }, icon('copy'), '複製到⋯'),
            h('button', {
                type: 'button',
                class: 'mc',
                title: '匯出成 zip',
                onclick: async () => {
                    try {
                        const path = await api.world_export({ id: instance.id, folder: w.folder });
                        toast(`已匯出到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
                        api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
                    } catch (error) {
                        fail(error);
                    }
                }
            }, icon('download'), '匯出'),
            h('button', {
                type: 'button',
                class: 'mc icon-btn danger',
                title: '刪除',
                'aria-label': '刪除',
                onclick: async () => {
                    if (!(await confirm(`要刪除世界「${w.name}」嗎？\n它會移到實例資料夾的 trash，需要時還能找回來。`, '刪除'))) return showWorlds(instance);
                    await api.world_delete({ id: instance.id, folder: w.folder }).then(() => toast(`已刪除「${w.name}」`)).catch(fail);
                    showWorlds(instance);
                }
            }, icon('trash'))))
            : [h('div', { class: 'empty-state' }, '這個實例還沒有任何單人世界。')]));
    };
    const importWorld = async (directory) => {
        const path = await pickFiles(directory ? { directory: true, multiple: false } : { multiple: false, filters: [{ name: '世界壓縮檔', extensions: ['zip'] }] }).catch(fail);
        if (!path) return;
        try {
            const name = await api.world_import({ id: instance.id, path });
            toast(`已匯入世界「${name}」`);
        } catch (error) {
            fail(error);
        }
        refresh();
    };
    refresh();
    await dialog({
        title: `世界管理｜${instance.name}`,
        wide: true,
        content: h('div', {},
            h('div', { class: 'toolbar' },
                h('button', { type: 'button', class: 'mc primary', onclick: () => importWorld(false) }, icon('plus'), '匯入 zip'),
                h('button', { type: 'button', class: 'mc', onclick: () => importWorld(true) }, icon('folder'), '匯入資料夾'),
                h('span', { class: 'spacer' }),
                h('button', { type: 'button', class: 'mc', onclick: () => openFolder(instance, 'saves') }, icon('folder'), '開啟存檔資料夾')),
            list,
            h('div', { class: 'hint' }, '遊戲執行中無法匯入、複製或刪除世界。'))
    });
}

async function copyWorld(instance, world) {
    const targets = store.instances.filter((i) => i.id !== instance.id);
    if (!targets.length) return toast('沒有其他實例可以複製過去', 'warn');
    const select = h('select', {}, ...targets.map((t) => h('option', { value: t.id }, `${t.name}（${t.mcVersion}）`)));
    const ok = await dialog({
        title: `複製「${world.name}」`,
        content: h('div', { class: 'field' }, h('label', {}, '複製到哪個實例'), select,
            h('div', { class: 'hint' }, '用比較舊的版本開啟新版本的世界可能會損壞，建議複製到相同或更新的版本。')),
        buttons: [{ label: '取消', value: false }, { label: '複製', value: true }]
    });
    if (ok === true) {
        const target = targets.find((t) => t.id === select.value);
        try {
            const name = await api.world_copy({ id: instance.id, folder: world.folder, target: target.id });
            toast(`已複製到「${target.name}」（${name}）`);
        } catch (error) {
            fail(error);
        }
    }
    return showWorlds(instance);
}

const BACKUP_KIND = { auto: '自動', manual: '手動', 'before-restore': '還原前' };

async function manageBackups(instance) {
    const list = h('div', { class: 'rows' }, h('div', { class: 'empty-state' }, '讀取中⋯'));
    const refresh = async () => {
        const backups = await api.list_backups({ id: instance.id }).catch((error) => (fail(error), []));
        clear(list).append(...(backups.length ? backups.map((b) => h('div', { class: 'row' },
            h('span', { class: 'row-icon' }, icon('archive')),
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, new Date(b.time * 1000).toLocaleString('zh-TW', { hour12: false })),
                h('div', { class: 'meta' }, `${BACKUP_KIND[b.kind] ?? b.kind}備份｜${fmtBytes(b.size)}`)),
            h('button', {
                type: 'button',
                class: 'mc',
                onclick: async () => {
                    if (!(await confirm(`要把存檔還原到 ${new Date(b.time * 1000).toLocaleString('zh-TW', { hour12: false })} 的狀態嗎？\n目前的存檔會先自動備份一份「還原前」備份。`, '還原'))) return manageBackups(instance);
                    try {
                        await api.restore_backup({ id: instance.id, file: b.file });
                        toast('存檔已還原');
                    } catch (error) {
                        fail(error);
                    }
                    manageBackups(instance);
                }
            }, '還原'),
            h('button', {
                type: 'button',
                class: 'mc icon-btn danger',
                title: '刪除',
                'aria-label': '刪除',
                onclick: async () => {
                    await api.delete_backup({ id: instance.id, file: b.file }).catch(fail);
                    refresh();
                }
            }, icon('trash'))))
            : [h('div', { class: 'empty-state' }, '還沒有任何備份。')]));
    };
    const content = h('div', {},
        h('div', { class: 'toolbar' },
            h('button', {
                type: 'button',
                class: 'mc primary',
                onclick: async (e) => {
                    e.currentTarget.disabled = true;
                    try {
                        await api.backup_now({ id: instance.id });
                        toast('已備份存檔');
                    } catch (error) {
                        fail(error);
                    }
                    e.target.closest('button').disabled = false;
                    refresh();
                }
            }, icon('save'), '立即備份'),
            h('span', { class: 'spacer' }),
            h('button', { type: 'button', class: 'mc', onclick: () => openFolder(instance, '../backups') }, icon('folder'), '開啟備份資料夾')),
        list,
        h('div', { class: 'hint' }, '可在「設定 → 啟動流程」開啟每次啟動前自動備份。'));
    refresh();
    await dialog({ title: `存檔備份｜${instance.name}`, content, wide: true });
}

export async function shareScreenshot(instance, shot) {
    const input = h('input', { type: 'text', maxlength: 60, placeholder: '介紹一下這張截圖（選填，最多 60 字）' });
    const go = await dialog({
        title: '投稿到玩家足跡',
        content: h('div', { class: 'share-shot' },
            h('img', { src: fileSrc(shot.path), alt: shot.name }),
            input,
            h('p', { class: 'hint' }, '投稿會用目前登入的正版帳號驗證身分，你的 Discord 需要先在遊戲內用 /discord link 綁定。管理員審核通過後，截圖會出現在官網的玩家足跡與首頁輪播。')),
        buttons: [{ label: '取消', value: false }, { label: '送出投稿', value: true }]
    });
    if (go === true) {
        toast('正在上傳截圖⋯');
        await api.share_screenshot({ id: instance.id, file: shot.name, description: input.value }).then((message) => toast(message), async (error) => {
            fail(error);
            if (/discord link/.test(String(error?.message ?? error)) && await confirm('你的帳號還沒綁定 Discord，要打開官網的綁定教學嗎？', '打開教學')) {
                openUrl(`https://www.brilliantw.net/${encodeURIComponent('連結帳號')}/`);
            }
        });
    }
}

async function showScreenshots(instance) {
    const grid = h('div', { class: 'shots' }, h('div', { class: 'empty-state' }, '讀取中⋯'));
    api.list_screenshots({ id: instance.id }).then((shots) => {
        clear(grid).append(...(shots.length ? shots.map((shot) => h('div', { class: 'shot' },
            h('button', { type: 'button', class: 'shot-open', title: shot.name, onclick: () => api.open_path({ path: shot.path }).catch(fail) },
                h('img', { src: fileSrc(shot.path), alt: shot.name, loading: 'lazy' })),
            h('div', { class: 'shot-foot' },
                h('span', {}, new Date(shot.time * 1000).toLocaleString('zh-TW', { hour12: false })),
                h('button', { type: 'button', class: 'mc', onclick: () => shareScreenshot(instance, shot).then(() => showScreenshots(instance)) }, icon('heart'), '投稿'))))
            : [h('div', { class: 'empty-state' }, '還沒有截圖。遊戲中按 F2 就能截圖。')]));
    }).catch(fail);
    await dialog({
        title: `截圖｜${instance.name}`,
        wide: true,
        content: h('div', {},
            h('div', { class: 'toolbar' },
                h('span', { class: 'toolbar-label' }, '點一下用系統預設程式開啟'),
                h('span', { class: 'spacer' }),
                h('button', { type: 'button', class: 'mc', onclick: () => openFolder(instance, 'screenshots') }, icon('folder'), '開啟截圖資料夾')),
            grid)
    });
}

async function createShortcut(instance) {
    const target = h('select', {},
        h('option', { value: '' }, '只開啟遊戲（進主畫面）'),
        ...(instance.general ? [] : [['s1', '炒飯服'], ['s2', '書碼服'], ['s3', '羽森服'], ['s4', '恐龍服']].map(([value, label]) =>
            h('option', { value }, `直接連線 ${label}`))));
    const ok = await dialog({
        title: `建立「${instance.name}」的桌面捷徑`,
        content: h('div', {},
            h('p', { class: 'hint' }, '在桌面放一個捷徑，點兩下就用這個實例直接開始遊戲，不必先操作啟動器。'),
            h('div', { class: 'field' }, h('label', {}, '開啟後'), target)),
        buttons: [{ label: '取消', value: false }, { label: '建立', value: true }]
    });
    if (ok !== true) return;
    try {
        const path = await api.create_shortcut({ id: instance.id, join: target.value || null });
        toast(`已建立捷徑：${path.split(/[\\/]/).pop()}`);
    } catch (error) {
        fail(error);
    }
}

async function exportDiagnostics(instance) {
    toast('正在打包診斷資料⋯');
    try {
        const path = await api.export_diagnostics({ id: instance.id });
        toast(`已匯出到「下載」資料夾：${path.split(/[\\/]/).pop()}\n裡面的登入憑證已自動隱藏，可以放心傳給管理員。`);
        api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
    } catch (error) {
        fail(error);
    }
}

function moreMenu(instance) {
    const item = (name, label, handler, extra = '') =>
        h('button', { type: 'button', role: 'menuitem', class: extra, onclick: (e) => { e.stopPropagation(); closeMenus(); handler(); } }, icon(name), label);
    const menu = h('div', { class: 'inst-menu', role: 'menu', hidden: true },
        item('map', '世界管理', () => showWorlds(instance)),
        item('save', '存檔備份', () => manageBackups(instance)),
        item('image', '截圖', () => showScreenshots(instance)),
        item('zap', '效能健檢', () => perfCheck(instance)),
        item('arrow-right', '升級版本⋯', () => pickUpgrade(instance)),
        item('folder', '開啟資料夾', () => openFolder(instance)),
        item('reload', '驗證並修復檔案', () => repairInstance(instance)),
        item('external-link', '建立桌面捷徑', () => createShortcut(instance)),
        item('bug', '匯出診斷包', () => exportDiagnostics(instance)),
        item('copy', '複製實例', () => duplicateInstance(instance)),
        item('trash', '刪除實例', () => removeInstance(instance), 'danger'));
    const toggle = h('button', {
        type: 'button',
        class: 'mc icon-btn',
        title: '更多',
        'aria-label': '更多',
        'aria-haspopup': 'menu',
        onclick: (e) => {
            e.stopPropagation();
            const open = menu.hidden;
            closeMenus();
            menu.hidden = !open;
            if (open) placeMenu(menu, toggle);
        }
    }, icon('more-horizontal'));
    return h('div', { class: 'inst-more' }, toggle, menu);
}

function placeMenu(menu, anchor) {
    const bounds = (anchor.closest('.page-wrap') ?? document.body).getBoundingClientRect();
    const rect = anchor.getBoundingClientRect();
    const above = rect.top - bounds.top - 12;
    const below = bounds.bottom - rect.bottom - 12;
    menu.style.maxHeight = '';
    const down = menu.scrollHeight > above && below > above;
    menu.classList.toggle('down', down);
    menu.style.maxHeight = `${Math.max(160, down ? below : above)}px`;
}

function closeMenus() {
    document.querySelectorAll('.inst-menu').forEach((m) => { m.hidden = true; });
}

async function playInstance(instance) {
    if (isPreparing()) return toast('正在準備遊戲，請等這次完成後再啟動', 'warn');
    await selectInstance(instance).catch(fail);
    location.hash = '#home';
    emit('launch');
}

async function manageMods(instance) {
    await selectInstance(instance).catch(fail);
    location.hash = '#mods';
}

function card(instance) {
    const active = selectedInstance()?.id === instance.id;
    const running = isRunning(instance.id);
    const fabric = instance.loader === 'fabric';
    const inject = fabric && !instance.general && (instance.injectRichPresence ?? store.settings?.injectRichPresence ?? true);
    const supported = store.versions?.modVersions;
    const unsupported = !instance.general && supported?.length && !supported.includes(instance.mcVersion);
    const tool = (name, label, handler, extra = '') =>
        h('button', {
            type: 'button',
            class: `mc icon-btn ${extra}`,
            title: label,
            'aria-label': label,
            onclick: (e) => { e.stopPropagation(); handler(); }
        }, icon(name));

    return h(
        'div',
        { class: `inst-card${active ? ' selected' : ''}`, onclick: () => selectInstance(instance).catch(fail) },
        h('div', { class: `inst-banner${instance.image ? ' has-image' : ''}`, style: instance.image ? `--cover:url("${fileSrc(instance.image)}")` : '' },
            instance.image ? null : icon(instanceIconName(instance.icon)),
            h('div', { class: 'inst-flags' },
                running ? h('span', { class: 'badge warn' }, '執行中') : null,
                active ? h('span', { class: 'badge' }, '使用中') : null)),
        h('div', { class: 'inst-body' },
            h('div', { class: 'inst-title', title: instance.name }, instance.name),
            h('div', { class: 'inst-tags' },
                h('span', { class: 'tag' }, `Minecraft ${instance.mcVersion}`),
                h('span', { class: 'tag' }, fabric ? `Fabric${instance.loaderVersion ? ` ${instance.loaderVersion}` : ''}` : '原版'),
                instance.general ? h('span', { class: 'tag' }, '一般實例') : null,
                inject ? h('span', { class: 'tag accent' }, 'Brilliant-Server-Mod') : null),
            h('div', { class: 'inst-meta' },
                icon('clock'),
                [fmtDuration(instance.playSeconds), instance.lastPlayed ? `上次 ${new Date(instance.lastPlayed * 1000).toLocaleDateString('zh-TW')}` : ''].filter(Boolean).join('｜')),
            unsupported
                ? h('div', { class: 'inst-warn' }, icon('warning-box'), '輝煌不支援這個版本',
                    store.versions?.recommended && fabric
                        ? h('button', { type: 'button', class: 'link', onclick: (e) => { e.stopPropagation(); upgradeFlow(instance, store.versions.recommended); } }, `升級到 ${store.versions.recommended}`)
                        : null)
                : null),
        h('div', { class: 'inst-actions' },
            h('button', {
                type: 'button',
                class: 'mc primary inst-play',
                onclick: (e) => { e.stopPropagation(); playInstance(instance); }
            }, icon(running ? 'close' : 'play'), running ? '結束遊戲' : '開始遊戲'),
            tool('edit', '設定', () => editInstance(instance)),
            fabric ? tool('zap', '模組與資源', () => manageMods(instance)) : null,
            moreMenu(instance))
    );
}

function render() {
    const box = clear($('#instance-list'));
    if (!store.instances.length) {
        box.append(h('div', { class: 'empty-state inst-empty' },
            icon('zap'),
            h('div', {}, '還沒有任何實例'),
            h('div', { class: 'hint' }, '實例是一套獨立的遊戲版本、模組與存檔。建議先一鍵建立輝煌實例。'),
            h('button', { type: 'button', class: 'mc primary', onclick: () => quickCreate().catch(fail) }, icon('plus'), '一鍵建立輝煌實例')));
        return;
    }
    box.append(...store.instances.map(card));
}

export function initInstances() {
    $('#btn-new-instance').addEventListener('click', guard(() => createInstance()));
    $('#btn-quick-instance').addEventListener('click', guard(() => quickCreate().catch(fail)));
    $('#btn-import').addEventListener('click', guard(() => importFlow().catch(fail)));
    $('#btn-refresh-instances').addEventListener('click', guard(async () => {
        store.versions = null;
        await loadInstances().catch(fail);
        await versionData().catch(() => {});
        render();
        toast('已重新整理');
    }));
    on('instances', render);
    document.addEventListener('click', closeMenus);
    on('settings', render);
    render();
    versionData().then(render, () => {});
}
