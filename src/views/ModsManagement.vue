<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, reactive, nextTick } from 'vue';
import { gamesList, appSettings } from '../store';
import {
  convertFileSrc,
  listenEvent,
  openFileDialog,
  createModGroup as apiCreateModGroup,
  setModGroupIcon as apiSetModGroupIcon,
  renameModGroup as apiRenameModGroup,
  deleteModGroup as apiDeleteModGroup,
  moveModToGroup as apiMoveModToGroup,
  deleteMod as apiDeleteMod,
  unwatchMods as apiUnwatchMods,
  previewModArchive as apiPreviewModArchive,
  installModArchive as apiInstallModArchive,
  watchMods as apiWatchMods,
  scanMods as apiScanMods,
  toggleMod as apiToggleMod,
  openInExplorer,
  openModGroupFolder as apiOpenModGroupFolder,
  openGameModsFolder as apiOpenGameModsFolder,
} from '../api';
import { Folder, Refresh, Picture, Search, Plus, Edit, Delete, FolderAdd, ArrowRight } from '@element-plus/icons-vue';
import { ElMessage, ElMessageBox } from 'element-plus';

interface ModInfo {
    id: string;
    name: string;
    enabled: boolean;
    path: string;
    relativePath: string;
    previewImages: string[];
    group: string;
    isDir: boolean;
    last_modified: number;
}

interface ArchivePreview {
    root_dirs: string[];
    file_count: number;
    has_ini: boolean;
    format: string;
}

// Context Menu State
const contextMenu = reactive({
    visible: false,
    x: 0,
    y: 0,
    type: 'mod' as 'mod' | 'group',
    target: null as any
});

const closeContextMenu = () => {
    contextMenu.visible = false;
};

// Group Management
const createNewGroup = async () => {
    try {
        const result = await ElMessageBox.prompt('请输入新分类名称', '新建分类', {
            confirmButtonText: '创建',
            cancelButtonText: '取消',
        }) as any;
        
        const value = result.value;
        
        if (value) {
            await apiCreateModGroup(selectedGame.value, value);
            ElMessage.success('分类创建成功');
            // Refresh logic usually handled by watcher, but manual refresh is safer
             fetchMods();
        }
    } catch {
        // User cancelled
    }
};

const subGroupDialog = reactive({
    visible: false,
    parentId: '',
    name: '',
    icon: ''
});

const openSubGroupDialog = (parentId: string) => {
    subGroupDialog.visible = true;
    subGroupDialog.parentId = parentId;
    subGroupDialog.name = '';
    subGroupDialog.icon = '';
};

const pickSubGroupIcon = async () => {
    const picked = await openFileDialog({
        multiple: false,
        filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'bmp', 'webp'] }]
    });
    if (picked) {
        subGroupDialog.icon = picked;
    }
};

const confirmSubGroup = async () => {
    if (!subGroupDialog.name) {
        ElMessage.warning('请输入子分类名称');
        return;
    }
    const newGroupPath = subGroupDialog.parentId ? `${subGroupDialog.parentId}/${subGroupDialog.name}` : subGroupDialog.name;
    try {
        await apiCreateModGroup(selectedGame.value, newGroupPath);
        if (subGroupDialog.icon) {
            try {
                await apiSetModGroupIcon(selectedGame.value, newGroupPath, subGroupDialog.icon);
            } catch (e: any) {
                ElMessage.warning('子分类创建成功，但图标设置失败: ' + e);
            }
        }
        ElMessage.success('子分类创建成功');
        subGroupDialog.visible = false;
        fetchMods();
    } catch (e: any) {
        ElMessage.error('创建失败: ' + e);
    }
};

const renameGroup = async (oldName: string) => {
    try {
        const result = await ElMessageBox.prompt('请输入新的分类名称', '重命名分类', {
            confirmButtonText: '确定',
            cancelButtonText: '取消',
            inputValue: oldName
        }) as any;
        
        const value = result.value;
        
        if (value && value !== oldName) {
             await apiRenameModGroup(selectedGame.value, oldName, value);
            ElMessage.success('分类重命名成功');
            if (selectedGroup.value === oldName) {
                selectedGroup.value = value;
            }
            fetchMods();
        }
    } catch {
        // User cancelled
    }
};

const deleteGroup = async (groupName: string) => {
    try {
        await ElMessageBox.confirm(
            `确定要删除分类 "${groupName}" 吗？这会将文件夹移动到回收站。`,
            '删除分类',
            {
                confirmButtonText: '删除',
                cancelButtonText: '取消',
                type: 'warning',
            }
        )
        
        await apiDeleteModGroup(selectedGame.value, groupName);
        
        ElMessage.success('分类已删除');
        if (selectedGroup.value === groupName) {
            selectedGroup.value = 'All';
        }
        fetchMods();
    } catch (e: any) {
        if (e !== 'cancel') {
             ElMessage.error(`删除失败: ${e}`);
        }
    }
};

const moveModToGroup = async (mod: ModInfo, groupName: string) => {
    try {
        await apiMoveModToGroup(selectedGame.value, mod.id, groupName);
        ElMessage.success(`移动到 ${groupName || 'Root'} 成功`);
        // fetchMods handled by watcher mostly
    } catch (e: any) {
        ElMessage.error('移动失败: ' + e);
    }
};

const deleteMod = async (mod: ModInfo) => {
    try {
        await ElMessageBox.confirm(
            `确定要删除 Mod "${mod.name}" 吗？这会将文件移动到回收站。`,
            '删除 Mod',
            {
                confirmButtonText: '删除',
                cancelButtonText: '取消',
                type: 'warning',
            }
        )
        
        await apiDeleteMod(selectedGame.value, mod.relativePath);
        
        ElMessage.success('Mod 已删除');
        // fetchMods handled by watcher
    } catch (e: any) {
        if (e !== 'cancel') {
             ElMessage.error(`删除失败: ${e}`);
        }
    }
};

const showGroupContextMenu = (e: MouseEvent, group: string) => {
    if (group === 'All' || group === 'Root') return;
    contextMenu.visible = true;
    contextMenu.x = e.clientX;
    contextMenu.y = e.clientY;
    contextMenu.type = 'group';
    contextMenu.target = group;
};

const showModContextMenu = (e: MouseEvent, mod: ModInfo) => {
    contextMenu.visible = true;
    contextMenu.x = e.clientX;
    contextMenu.y = e.clientY;
    contextMenu.type = 'mod';
    contextMenu.target = mod;
};


interface GroupInfo {
    id: string; // Full path
    name: string; // Display name
    iconPath?: string;
}

const loading = ref(false);
const mods = ref<ModInfo[]>([]);
const availableGroups = ref<GroupInfo[]>([]);
const selectedGame = ref('');
const searchQuery = ref('');
const selectedGroup = ref('All');
// Sorting state
const GROUP_ORDER_KEY = 'ssmt4_group_orders_v1';
const GROUP_EXPANDED_KEY = 'ssmt4_group_expanded_v1';

const ORDER_STORAGE_KEY = 'ssmt4_mod_manual_orders_v1';

function loadManualOrders() {
    if (typeof localStorage === 'undefined') {
        return {} as Record<string, Record<string, string[]>>;
    }
    try {
        const raw = localStorage.getItem(ORDER_STORAGE_KEY);
        if (raw) {
            const parsed = JSON.parse(raw);
            if (parsed && typeof parsed === 'object') {
                return parsed;
            }
        }
    } catch (e) {
        console.warn('Failed to load manual orders', e);
    }
    return {} as Record<string, Record<string, string[]>>;
}

const manualOrders = ref<Record<string, Record<string, string[]>>>(loadManualOrders());
const draggingOrderId = ref<string | null>(null);
const dragOverId = ref<string | null>(null);
const manualSortState = reactive({
    active: false,
    startX: 0,
    startY: 0,
    hasMoved: false,
    mod: null as ModInfo | null,
});
let manualSortGroupHover: HTMLElement | null = null;

// Group manual order state
const ROOT_PARENT_ID = '__ROOT__';

function loadGroupOrders() {
    if (typeof localStorage === 'undefined') return {} as Record<string, Record<string, string[]>>;
    try {
        const raw = localStorage.getItem(GROUP_ORDER_KEY);
        if (raw) {
            const parsed = JSON.parse(raw);
            if (parsed && typeof parsed === 'object') return parsed;
        }
    } catch (e) {
        console.warn('Failed to load group orders', e);
    }
    return {} as Record<string, Record<string, string[]>>;
}

const groupOrders = ref<Record<string, Record<string, string[]>>>(loadGroupOrders());
const groupDragState = reactive({
    active: false,
    startX: 0,
    startY: 0,
    hasMoved: false,
    sourceId: '' as string,
    targetId: null as string | null,
    sourceParent: '' as string,
});
const groupHoverId = ref<string | null>(null);
const loadExpandedState = () => {
    if (typeof localStorage === 'undefined') return {} as Record<string, string[]>;
    try {
        const raw = localStorage.getItem(GROUP_EXPANDED_KEY);
        if (raw) {
            const parsed = JSON.parse(raw);
            if (parsed && typeof parsed === 'object') {
                console.log('[GroupExpanded] Loaded from storage', parsed);
                return parsed;
            }
        }
    } catch (e) {
        console.warn('Failed to load expanded groups', e);
    }
    console.log('[GroupExpanded] No stored data, using empty state');
    return {} as Record<string, string[]>;
};

const expandedState = ref<Record<string, string[]>>(loadExpandedState());
const expandedKeys = ref<string[]>([]);
const groupTreeRef = ref();

const persistManualOrders = () => {
    if (typeof localStorage === 'undefined') return;
    try {
        localStorage.setItem(ORDER_STORAGE_KEY, JSON.stringify(manualOrders.value));
    } catch (e) {
        console.warn('Failed to save manual orders', e);
    }
};

const persistGroupOrders = () => {
    if (typeof localStorage === 'undefined') return;
    try {
        localStorage.setItem(GROUP_ORDER_KEY, JSON.stringify(groupOrders.value));
    } catch (e) {
        console.warn('Failed to save group orders', e);
    }
};

const persistExpandedState = () => {
    if (typeof localStorage === 'undefined') return;
    try {
        localStorage.setItem(GROUP_EXPANDED_KEY, JSON.stringify(expandedState.value));
        console.log('[GroupExpanded] Saved', expandedState.value);
    } catch (e) {
        console.warn('Failed to save expanded state', e);
    }
};

const getGroupParent = (id: string) => {
    const parts = id.split('/');
    if (parts.length <= 1) return ROOT_PARENT_ID;
    return parts.slice(0, -1).join('/');
};

const sanitizeGroupOrder = (game: string, parentId: string, childrenIds: string[]) => {
    if (!groupOrders.value[game]) groupOrders.value[game] = {};
    const existing = groupOrders.value[game][parentId] || [];
    const valid = new Set(childrenIds);
    const filtered = existing.filter(id => valid.has(id));
    const filteredSet = new Set(filtered);
    const missing = childrenIds.filter(id => !filteredSet.has(id)).sort((a, b) => a.localeCompare(b));
    const next = [...filtered, ...missing];
    const changed = existing.length !== next.length || existing.some((id, idx) => id !== next[idx]);
    if (changed) {
        groupOrders.value[game][parentId] = next;
        persistGroupOrders();
    }
    return next;
};

const sanitizeExpanded = (game: string) => {
    const allIds = new Set(availableGroups.value.map(g => g.id));
    // Safety: If no groups available (e.g. data not loaded or error), skip sanitization
    // so we don't wipe the user's saved state accidentally.
    if (allIds.size === 0) {
         return [...(expandedState.value[game] || [])];
    }

    const current = expandedState.value[game] || [];
    const filtered = current.filter(id => allIds.has(id));
    const changed = filtered.length !== current.length;
    if (!expandedState.value[game]) {
        expandedState.value[game] = filtered;
        console.log('[GroupExpanded] Init bucket for', game, filtered);
        persistExpandedState();
    } else if (changed) {
        expandedState.value[game] = filtered;
        console.log('[GroupExpanded] Filtered missing nodes for', game, filtered);
        persistExpandedState();
    }
    console.log('[GroupExpanded] Sanitize result for', game, expandedState.value[game]);
    return [...(expandedState.value[game] || [])];
};

const applyExpandedToTree = () => {
    nextTick(() => {
        try {
            const tree = groupTreeRef.value as any;
            const keys = [...expandedKeys.value];
            console.log('[GroupExpanded] applyExpandedToTree', { keys, hasTree: !!tree });
            if (!tree) {
                console.warn('[GroupExpanded] tree ref missing, skip apply');
                return;
            }
            if (typeof tree.setExpandedKeys === 'function') {
                tree.setExpandedKeys(keys);
                return;
            }
            if (tree.store && typeof tree.store.setDefaultExpandedKeys === 'function') {
                tree.store.setDefaultExpandedKeys(keys);
                return;
            }
            // Fallback: manually expand nodes
            if (typeof tree.getNode === 'function') {
                keys.forEach(id => {
                    const node = tree.getNode(id);
                    if (node) node.expanded = true;
                });
            } else if (tree.store && typeof tree.store.getNode === 'function') {
                keys.forEach(id => {
                    const node = tree.store.getNode(id);
                    if (node) node.expanded = true;
                });
            } else {
                console.warn('[GroupExpanded] No expand APIs available');
            }
        } catch (e) {
            console.warn('Failed to apply expanded keys', e);
        }
    });
};

const applyGroupOrder = (parentId: string, nodes: any[]) => {
    const game = selectedGame.value;
    if (!game) return nodes;
    const ids = nodes.map(n => n.id);
    const orderList = sanitizeGroupOrder(game, parentId, ids);
    const idxMap = new Map(orderList.map((id, idx) => [id, idx]));
    return [...nodes].sort((a, b) => {
        const ia = idxMap.get(a.id) ?? Number.MAX_SAFE_INTEGER;
        const ib = idxMap.get(b.id) ?? Number.MAX_SAFE_INTEGER;
        return ia - ib;
    });
};

const getOrderContext = () => {
    if (!selectedGame.value) return null;
    return { game: selectedGame.value, group: selectedGroup.value || 'All' };
};

const getModsByGroup = (group: string) => {
    if (group === 'All') return mods.value;
    if (group === 'Root') return mods.value.filter(m => m.group === 'Root');
    return mods.value.filter(m => m.group === group);
};

const compareDefault = (a: ModInfo, b: ModInfo) => {
    let cmp = (b.last_modified || 0) - (a.last_modified || 0);
    if (cmp !== 0) return cmp;
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
};

const sanitizeOrderForContext = (game: string, group: string) => {
    const groupMods = getModsByGroup(group);
    const existing = manualOrders.value[game]?.[group] || [];
    const validIds = new Set(groupMods.map(m => m.id));
    const filteredExisting = existing.filter(id => validIds.has(id));
    const existingSet = new Set(filteredExisting);
    const missing = groupMods
        .filter(m => !existingSet.has(m.id))
        .sort(compareDefault)
        .map(m => m.id);
    const next = [...filteredExisting, ...missing];
    if (!manualOrders.value[game]) manualOrders.value[game] = {};
    const prev = manualOrders.value[game][group] || [];
    const changed = prev.length !== next.length || prev.some((id, idx) => id !== next[idx]);
    if (changed) {
        manualOrders.value[game][group] = next;
        persistManualOrders();
    }
    return next;
};

const getCurrentOrderList = () => {
    const ctx = getOrderContext();
    if (!ctx) return [] as string[];
    return sanitizeOrderForContext(ctx.game, ctx.group);
};

const applyManualReorder = (dragId: string, targetId: string) => {
    const ctx = getOrderContext();
    if (!ctx) return;
    sanitizeOrderForContext(ctx.game, ctx.group);
    const order = manualOrders.value[ctx.game][ctx.group] || [];
    const from = order.indexOf(dragId);
    const to = order.indexOf(targetId);
    if (from === -1 || to === -1) return;
    const next = [...order];
    next.splice(from, 1);
    next.splice(to, 0, dragId);
    manualOrders.value[ctx.game][ctx.group] = next;
    persistManualOrders();
};

// Install Dialog State
const showInstallDialog = ref(false);
const installForm = reactive({
    archivePath: '',
    modName: '',
    targetGroup: '',
    password: ''
});
const installPreview = ref<ArchivePreview | null>(null);
const isInstalling = ref(false);

// Sidebar Resizing
const sidebarWidth = ref(220);
const isResizing = ref(false);

const startResize = (e: MouseEvent) => {
    isResizing.value = true;
    const startX = e.clientX;
    const startWidth = sidebarWidth.value;

    const doResize = (moveEvent: MouseEvent) => {
        const diff = moveEvent.clientX - startX;
        const newWidth = startWidth + diff;
        if (newWidth >= 150 && newWidth <= 500) {
            sidebarWidth.value = newWidth;
        }
    };

    const stopResize = () => {
        isResizing.value = false;
        document.removeEventListener('mousemove', doResize);
        document.removeEventListener('mouseup', stopResize);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
    };

    document.addEventListener('mousemove', doResize);
    document.addEventListener('mouseup', stopResize);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
};

// Native DnD (kept for completeness) + Manual fallback
const draggingMod = ref<ModInfo | null>(null);

// Global debug listeners (removed on unmount)
let globalDragOverLogger: ((e: DragEvent) => void) | null = null;
let globalDragEnterLogger: ((e: DragEvent) => void) | null = null;

const onCardMouseDownWrapper = (e: MouseEvent, mod: ModInfo) => {
    onManualSortMouseDown(e, mod);
};

const onDragEnter = (e: DragEvent) => {
    console.log('[DragEnter]', e.currentTarget);
    const target = (e.currentTarget as HTMLElement);
    target.classList.add('drag-over');
};

const onDragOver = (e: DragEvent) => {
    e.preventDefault(); // Necessary to allow dropping
    
    if (e.dataTransfer) {
        e.dataTransfer.dropEffect = 'move';
    }
    
    const target = (e.currentTarget as HTMLElement);
    if (!target.classList.contains('drag-over')) {
        target.classList.add('drag-over');
    }
};

const onDragLeave = (e: DragEvent) => {
    const target = (e.currentTarget as HTMLElement);
    // Fix: Only remove class if we are actually leaving the element, 
    // not just entering a child element (like the text span or icon)
    const related = e.relatedTarget as Node | null;
    if (target.contains(related)) {
        return;
    }
    target.classList.remove('drag-over');
};

const onDrop = async (e: DragEvent, targetGroupId: string) => {
    e.preventDefault();
    const target = (e.currentTarget as HTMLElement);
    target.classList.remove('drag-over');

    const rawData = e.dataTransfer?.getData('text/plain');
    const mod = draggingMod.value;

    console.log('[Drop]', { targetGroupId, rawData, modId: mod?.id });

    if (mod && (mod.id === rawData || !rawData)) {
        const modId = mod.id;
        
        if (mod.group === targetGroupId) return;
        if (targetGroupId === 'All') {
            return; 
        }
    
        try {
            await apiMoveModToGroup(selectedGame.value, modId, targetGroupId);
            
            ElMessage.success({
                message: `已移动到 ${targetGroupId === 'Root' ? '未分类' : targetGroupId}`,
                offset: 48 
            });
        } catch (e: any) {
             ElMessage.error({
                message: `移动失败: ${e}`,
                offset: 48
            });
        } finally {
            draggingMod.value = null;
            document.body.style.userSelect = '';
        }
    } else {
        console.warn('[Drop] No mod captured or ID mismatch', { rawData, dragging: mod?.id });
    }
};

const onManualSortMouseDown = (e: MouseEvent, mod: ModInfo) => {
    if (e.button !== 0) return;
    manualSortState.active = true;
    manualSortState.startX = e.clientX;
    manualSortState.startY = e.clientY;
    manualSortState.hasMoved = false;
    manualSortState.mod = mod;
    draggingOrderId.value = mod.id;
    document.addEventListener('mousemove', onManualSortMouseMove);
    document.addEventListener('mouseup', onManualSortMouseUp);
};

const setManualSortGroupHover = (el: HTMLElement | null) => {
    if (manualSortGroupHover && manualSortGroupHover !== el) {
        manualSortGroupHover.classList.remove('drag-over');
    }
    if (el && manualSortGroupHover !== el) {
        el.classList.add('drag-over');
    }
    manualSortGroupHover = el;
};

const onManualSortMouseMove = (e: MouseEvent) => {
    if (!manualSortState.active || !manualSortState.mod) return;
    const dx = e.clientX - manualSortState.startX;
    const dy = e.clientY - manualSortState.startY;
    if (!manualSortState.hasMoved && Math.hypot(dx, dy) > 3) {
        manualSortState.hasMoved = true;
        document.body.style.userSelect = 'none';
    }

    if (manualSortState.hasMoved) {
        const el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
        const groupEl = el?.closest?.('[data-group-id]') as HTMLElement | null;
        if (groupEl) {
            setManualSortGroupHover(groupEl);
            dragOverId.value = null; // don't show card hover while over sidebar
        } else {
            setManualSortGroupHover(null);
            const card = el?.closest?.('.mod-card') as HTMLElement | null;
            const targetId = card?.dataset?.modId || null;
            dragOverId.value = targetId;
        }
    }
};

function resetManualSortState() {
    manualSortState.active = false;
    manualSortState.hasMoved = false;
    manualSortState.mod = null;
    draggingOrderId.value = null;
    dragOverId.value = null;
    setManualSortGroupHover(null);
    document.body.style.userSelect = '';
}

const onManualSortMouseUp = (e: MouseEvent) => {
    document.removeEventListener('mousemove', onManualSortMouseMove);
    document.removeEventListener('mouseup', onManualSortMouseUp);

    if (!manualSortState.active || !manualSortState.mod) {
        resetManualSortState();
        return;
    }

    if (manualSortState.hasMoved) {
        const el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
        const groupEl = el?.closest?.('[data-group-id]') as HTMLElement | null;
        const targetGroupId = groupEl?.dataset.groupId;

        if (targetGroupId && targetGroupId !== 'All' && manualSortState.mod.group !== targetGroupId) {
            moveModToGroup(manualSortState.mod, targetGroupId);
            resetManualSortState();
            return;
        }

        const card = el?.closest?.('.mod-card') as HTMLElement | null;
        const targetId = card?.dataset?.modId || null;
        if (targetId && targetId !== manualSortState.mod.id) {
            applyManualReorder(manualSortState.mod.id, targetId);
        }
    }

    resetManualSortState();
};

// Watcher cleanup
let unlistenFileChange: (() => void) | null = null;
let unlistenDrop: (() => void) | null = null;
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

// Initialize selected game from store if possible
onMounted(async () => {
    // ... existing init code ...
    console.log('[GroupExpanded] onMounted start');
    if (appSettings.currentConfigName && gamesList.find(g => g.name === appSettings.currentConfigName)) {
        selectedGame.value = appSettings.currentConfigName;
    } else if (gamesList.length > 0) {
        selectedGame.value = gamesList[0].name;
    }
    
    // Listen for file drops (no-op in web mode)
    unlistenDrop = await listenEvent('tauri://drag-drop', async (event: any) => {
        const payload = event.payload;
        if (payload.paths && payload.paths.length > 0) {
            handleFileDrop(payload.paths[0]);
        }
    });

    // Start listening for file changes (no-op in web mode)
    unlistenFileChange = await listenEvent('mod-filesystem-changed', () => {
        // Debounce the refresh
        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
            console.log("File system changed, refreshing...");
            // Silent refresh (no loading spinner to avoid flickering)
            silentRefresh();
        }, 500); // 500ms debounce
    });
    
    if (selectedGame.value) {
        // Initial load
        startWatching(selectedGame.value);
    }

    // Debug: log global dragenter/dragover to see if events fire anywhere
    globalDragOverLogger = (e: DragEvent) => {
        // Only log for our page container to reduce noise
        const t = e.target as HTMLElement | null;
        if (t && t.closest && t.closest('.mod-manager')) {
            console.log('[Global dragover]', t.className);
        }
    };
    globalDragEnterLogger = (e: DragEvent) => {
        const t = e.target as HTMLElement | null;
        if (t && t.closest && t.closest('.mod-manager')) {
            console.log('[Global dragenter]', t.className);
        }
    };
    document.addEventListener('dragover', globalDragOverLogger);
    document.addEventListener('dragenter', globalDragEnterLogger);
});

onUnmounted(() => {
    if (unlistenFileChange) unlistenFileChange();
    if (unlistenDrop) unlistenDrop();
    // Stop watching backend? 
    // Ideally yes, but changing pages shouldn't necessarily stop watching if we want background updates. 
    // But for performance, let's stop it.
    apiUnwatchMods().catch((e: any) => console.error(e));

    if (globalDragOverLogger) document.removeEventListener('dragover', globalDragOverLogger);
    if (globalDragEnterLogger) document.removeEventListener('dragenter', globalDragEnterLogger);
});

watch(selectedGame, (newVal) => {
    if (newVal) {
        startWatching(newVal);
        selectedGroup.value = 'All';
    }
});

watch(() => appSettings.currentConfigName, (newVal) => {
    if (newVal && gamesList.find(g => g.name === newVal)) {
        selectedGame.value = newVal;
    }
}, { immediate: true });

watch([mods, selectedGame, selectedGroup], () => {
    const ctx = getOrderContext();
    if (ctx) sanitizeOrderForContext(ctx.game, ctx.group);
}, { immediate: true });

watch(availableGroups, () => {
    const game = selectedGame.value;
    if (!game) return;
    // Build children map per parent for sanitization
    const buckets: Record<string, string[]> = {};
    availableGroups.value.forEach(g => {
        const parent = getGroupParent(g.id);
        if (!buckets[parent]) buckets[parent] = [];
        buckets[parent].push(g.id);
    });
    Object.entries(buckets).forEach(([parent, ids]) => {
        sanitizeGroupOrder(game, parent, ids);
    });
    // Expanded keys sanitize & apply (fresh copy to trigger update)
    expandedKeys.value = [...sanitizeExpanded(game)];
    console.log('[GroupExpanded] Apply after groups refresh', expandedKeys.value);
    applyExpandedToTree();
}, { immediate: true });

watch(expandedKeys, () => {
    console.log('[GroupExpanded] expandedKeys watcher', expandedKeys.value);
    applyExpandedToTree();
});

const handleFileDrop = async (path: string) => {
    // Check extension
    const lower = path.toLowerCase();
    if (lower.endsWith('.zip') || lower.endsWith('.7z')) {
        installForm.archivePath = path;
        
        // Guess initial name from filename
        const filename = path.split(/[\\/]/).pop() || 'New Mod';
        installForm.modName = filename.replace(/\.(zip|7z|rar)/i, '');
        
        // Default group: if 'Root' or 'All' is selected, default to 'Default'
        // If a specific group is selected, use that.
        installForm.targetGroup = (selectedGroup.value === 'All' || selectedGroup.value === 'Root') ? 'Default' : selectedGroup.value;
        installForm.password = '';
        
        // Load Preview
        try {
            loading.value = true;
            installPreview.value = await apiPreviewModArchive(path);
            showInstallDialog.value = true;
        } catch (e: any) {
            ElMessage.error({
                message: `无法读取压缩包: ${e}`,
                offset: 48
            });
        } finally {
            loading.value = false;
        }
    } else if (lower.endsWith('.rar')) {
         installForm.archivePath = path;
        const filename = path.split(/[\\/]/).pop() || 'New Mod';
        installForm.modName = filename.replace(/\.(zip|7z|rar)/i, '');
        installForm.targetGroup = (selectedGroup.value === 'All' || selectedGroup.value === 'Root') ? 'Default' : selectedGroup.value;
        installForm.password = '';

         try {
            loading.value = true;
            installPreview.value = await apiPreviewModArchive(path);
            showInstallDialog.value = true;
        } catch (e: any) {
            // Show raw error if it's "not supported" to include details
            // Add offset to avoid titlebar
            ElMessage.error({
                message: `RAR 读取失败: ${e}`,
                offset: 48
            });
        } finally {
            loading.value = false;
        }
    }
};

const confirmInstall = async () => {
    if (!installForm.modName) {
        ElMessage.warning({ message: '请输入 Mod 名称', offset: 48 });
        return;
    }
    
    isInstalling.value = true;
    try {
        await apiInstallModArchive(
            selectedGame.value,
            installForm.archivePath,
            installForm.modName,
            installForm.targetGroup,
            installForm.password || null
        );
        ElMessage.success({ message: '安装成功！', offset: 48 });
        showInstallDialog.value = false;
        // Refresh handled by watcher
    } catch (e) {
        ElMessage.error({ message: `安装失败: ${e}`, offset: 48 });
    } finally {
        isInstalling.value = false;
    }
};


// ... existing code ...

const startWatching = async (gameName: string) => {
    loading.value = true;
    try {
        // First load data
        await refreshMods(gameName);
        
        // Restore expanded state after data is loaded
        expandedKeys.value = sanitizeExpanded(gameName);
        console.log('[GroupExpanded] Restored state after refresh', gameName, expandedKeys.value);
        applyExpandedToTree();

        console.log('[GroupExpanded] After refreshMods', gameName, { groups: availableGroups.value.map(g => g.id), expandedKeys: expandedKeys.value });
        // Then start watching (which might fail if folder doesn't exist, but that's ok)
        await apiWatchMods(gameName);
    } catch (error) {
        console.error('Failed to start watching:', error);
    } finally {
        loading.value = false;
    }
};

const silentRefresh = async () => {
    if (!selectedGame.value) return;
    try {
        const result = await apiScanMods(selectedGame.value) as unknown as { mods: ModInfo[], groups: GroupInfo[] };
        mods.value = result.mods;
        availableGroups.value = result.groups;
    } catch (e) {
        console.error("Silent refresh failed", e);
    }
}

const refreshMods = async (gameName: string) => {
    try {
        const result = await apiScanMods(gameName) as unknown as { mods: ModInfo[], groups: GroupInfo[] };
        mods.value = result.mods;
        availableGroups.value = result.groups;
    } catch (error) {
        console.error('Failed to scan mods:', error);
        ElMessage.error(`扫描失败: ${error}`);
        mods.value = [];
        availableGroups.value = [];
    }
};

// Removed old fetchMods, replaced by refreshMods/startWatching logics
const fetchMods = () => {
    if (selectedGame.value) startWatching(selectedGame.value);
};

const toggleMod = async (mod: ModInfo) => {
    // Optimistic UI update is risky here if renaming fails, but let's try
    // Better to wait for server response
    const originalState = mod.enabled;
    const targetState = !originalState; // We want to toggle
    
    try {
        await apiToggleMod(selectedGame.value, mod.relativePath, targetState);
        
        // Refresh list to get new paths
        await silentRefresh();
        ElMessage.success(`${mod.name} ${targetState ? '已启用' : '已禁用'}`);
    } catch (error) {
        console.error('Failed to toggle mod:', error);
        // creating the mod object implies it exists in memory, we might need a revert if we did optimistic
        ElMessage.error(`操作失败: ${error}`);
    }
};

const openModFolder = async (path: string) => {
    try {
        await openInExplorer(path);
    } catch (error) {
        console.error(error);
    }
};

const openGameFolder = async () => {
    try {
        await apiOpenGameModsFolder(selectedGame.value);
    } catch (error) {
        console.error(error);
    }
}

// Computed Properties
const groups = computed(() => {
    // Map of groupID -> GroupInfo
    const map = new Map<string, GroupInfo>();
    
    // Add known groups from backend
    availableGroups.value.forEach(g => {
        map.set(g.id, g);
    });

    // Add implicit groups from mods
    mods.value.forEach(m => {
        if (m.group && m.group !== "Root" && !map.has(m.group)) {
            // Split slash name if we want friendly name for implicit groups
            // ModInfo.group is the full path ID now
            const parts = m.group.split('/');
            const name = parts[parts.length - 1];
            map.set(m.group, { id: m.group, name: name });
        }
    });

    // Sort by ID is usually fine for hierarchy
    const list = Array.from(map.values()).sort((a, b) => a.id.localeCompare(b.id));

    return [{ id: 'All', name: '全部' }, ...list];
});

const groupTree = computed(() => {
    const nodeMap = new Map<string, any>();
    const buckets: Record<string, any[]> = {};

    const sorted = [...(groups.value || [])]
        .filter(g => g.id !== 'All' && g.id !== 'Root')
        .sort((a, b) => a.id.split('/').length - b.id.split('/').length);

    sorted.forEach(g => {
        const parts = g.id.split('/');
        const name = parts[parts.length - 1];
        const parentId = getGroupParent(g.id);

        const node = {
            id: g.id,
            label: name,
            children: [],
            icon: g.iconPath,
            count: mods.value.filter(m => m.group === g.id).length
        };

        nodeMap.set(g.id, node);
        if (!buckets[parentId]) buckets[parentId] = [];
        buckets[parentId].push(node);
    });

    // Apply ordering within each sibling bucket
    Object.keys(buckets).forEach(parentId => {
        buckets[parentId] = applyGroupOrder(parentId, buckets[parentId]);
    });

    // Build tree by attaching children to parents
    Object.entries(buckets).forEach(([parentId, children]) => {
        if (parentId === ROOT_PARENT_ID) return;
        const parent = nodeMap.get(parentId);
        if (parent) parent.children = children;
    });

    // Root nodes are those under ROOT_PARENT_ID
    return buckets[ROOT_PARENT_ID] || [];
});

const setGroupIcon = async (groupPath: string) => {
     try {
        const selected = await openFileDialog({
            multiple: false,
            filters: [{
                name: 'Image',
                extensions: ['png', 'jpg', 'jpeg', 'bmp', 'webp']
            }]
        });

        if (selected) {
            await apiSetModGroupIcon(selectedGame.value, groupPath, selected);
            ElMessage.success('图标设置成功');
            // Little hack to refresh image cache? 
            // Usually fetchMods -> rescans -> returns new icon list. 
            // Browser might cache image. convertFileSrc usually handles it? 
            // Sometimes need timestamp query.
            await fetchMods();
        }
    } catch (e: any) {
        ElMessage.error('设置图标失败: ' + e);
    }
};

const reorderGroup = (sourceId: string, targetId: string) => {
    const game = selectedGame.value;
    if (!game) return;
    if (sourceId === targetId) return;
    const sourceParent = getGroupParent(sourceId);
    const targetParent = getGroupParent(targetId);
    if (sourceParent !== targetParent) return; // forbid cross-level moves
    const siblings = availableGroups.value
        .filter(g => getGroupParent(g.id) === sourceParent)
        .map(g => g.id);
    sanitizeGroupOrder(game, sourceParent, siblings);
    const order = groupOrders.value[game][sourceParent] || [];
    const from = order.indexOf(sourceId);
    const to = order.indexOf(targetId);
    if (from === -1 || to === -1) return;
    const next = [...order];
    next.splice(from, 1);
    next.splice(to, 0, sourceId);
    groupOrders.value[game][sourceParent] = next;
    persistGroupOrders();
};

const onGroupExpand = (data: any) => {
    const game = selectedGame.value;
    if (!game) return;
    const id = data.id;
    const set = new Set(expandedKeys.value);
    if (!set.has(id)) {
        set.add(id);
        expandedKeys.value = Array.from(set);
        expandedState.value[game] = expandedKeys.value;
        console.log('[GroupExpanded] Expanded', id, '->', expandedKeys.value);
        persistExpandedState();
    }
};

const onGroupCollapse = (data: any) => {
    const game = selectedGame.value;
    if (!game) return;
    const id = data.id;
    const set = new Set(expandedKeys.value);
    if (set.has(id)) {
        set.delete(id);
        expandedKeys.value = Array.from(set);
        expandedState.value[game] = expandedKeys.value;
        console.log('[GroupExpanded] Collapsed', id, '->', expandedKeys.value);
        persistExpandedState();
    }
};

const onGroupMouseDown = (e: MouseEvent, groupId: string) => {
    if (groupId === 'All' || groupId === 'Root') return;
    if (e.button !== 0) return;
    groupDragState.active = true;
    groupDragState.startX = e.clientX;
    groupDragState.startY = e.clientY;
    groupDragState.hasMoved = false;
    groupDragState.sourceId = groupId;
    groupDragState.sourceParent = getGroupParent(groupId);
    groupDragState.targetId = null;
    document.addEventListener('mousemove', onGroupMouseMove);
    document.addEventListener('mouseup', onGroupMouseUp);
};

const onGroupMouseMove = (e: MouseEvent) => {
    if (!groupDragState.active) return;
    const dx = e.clientX - groupDragState.startX;
    const dy = e.clientY - groupDragState.startY;
    if (!groupDragState.hasMoved && Math.hypot(dx, dy) > 3) {
        groupDragState.hasMoved = true;
        document.body.style.userSelect = 'none';
    }
    if (groupDragState.hasMoved) {
        const el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
        const node = el?.closest?.('[data-group-id]') as HTMLElement | null;
        const targetId = node?.dataset.groupId || null;
        const targetParent = node?.dataset.parentId || null;
        if (targetId && targetParent === groupDragState.sourceParent) {
            groupHoverId.value = targetId;
            groupDragState.targetId = targetId;
        } else {
            groupHoverId.value = null;
            groupDragState.targetId = null;
        }
    }
};

const onGroupMouseUp = () => {
    document.removeEventListener('mousemove', onGroupMouseMove);
    document.removeEventListener('mouseup', onGroupMouseUp);

    if (groupDragState.hasMoved && groupDragState.targetId) {
        reorderGroup(groupDragState.sourceId, groupDragState.targetId);
    }
    resetGroupDrag();
};

const resetGroupDrag = () => {
    groupDragState.active = false;
    groupDragState.hasMoved = false;
    groupDragState.sourceId = '';
    groupDragState.targetId = null;
    groupDragState.sourceParent = '';
    groupHoverId.value = null;
    document.body.style.userSelect = '';
};

const openModGroupFolder = async (groupPath: string) => {
    try {
        await apiOpenModGroupFolder(selectedGame.value, groupPath);
    } catch (e: any) {
        ElMessage.error('无法打开文件夹: ' + e);
    }
};

const filteredMods = computed(() => {
    let result = [...mods.value];

    if (selectedGroup.value !== 'All') {
        result = result.filter(m => m.group === selectedGroup.value);
    }

    if (searchQuery.value) {
        const query = searchQuery.value.toLowerCase();
        result = result.filter(m => m.name.toLowerCase().includes(query));
    }

    const orderList = getCurrentOrderList();
    const orderMap = new Map(orderList.map((id, idx) => [id, idx]));

    return [...result].sort((a, b) => {
        const ia = orderMap.get(a.id);
        const ib = orderMap.get(b.id);
        if (ia !== undefined && ib !== undefined) return ia - ib;
        if (ia !== undefined) return -1;
        if (ib !== undefined) return 1;
        return compareDefault(a, b);
    });
});

const getPreviewUrl = (mod: ModInfo) => {
    if (mod.previewImages && mod.previewImages.length > 0) {
        return convertFileSrc(mod.previewImages[0]);
    }
    return ''; // Placeholder handled by UI
};

const getGroupIcon = (groupId: string) => {
    if(!groupId || groupId === 'Root') return null;
    const group = availableGroups.value.find(g => g.id === groupId);
    // Loop through implicit groups if not found? 
    // availableGroups usually contains all groups found by scanner.
    return group?.iconPath;
};

</script>

<template>
  <div class="page-container mod-manager">
    <!-- Header Toolbar -->
    <div class="toolbar glass-panel">
        <div class="left-tools">
            <el-input
                v-model="searchQuery"
                placeholder="搜索 Mod..."
                :prefix-icon="Search"
                style="width: 240px"
                clearable
            />
        </div>

        <div class="right-tools">
            <el-button @click="openGameFolder" :icon="Folder" plain>文件夹</el-button>
            <el-button @click="fetchMods" :icon="Refresh" :loading="loading" circle type="primary" plain></el-button>
        </div>
    </div>

    <div class="main-content" @click="closeContextMenu" @contextmenu="closeContextMenu">
        <!-- Sidebar Filter -->
           <div class="sidebar glass-panel" :style="{ width: sidebarWidth + 'px' }"
               @dragenter.stop.prevent
               @dragover.stop.prevent
               @drop.stop.prevent>
            <div class="sidebar-header">
                <span class="title">分类列表</span>
                <el-button :icon="Plus" circle size="small" @click.stop="createNewGroup" text bg />
            </div>
            <div class="group-list">
                <div 
                    class="group-item" 
                    :class="{ active: selectedGroup === 'All' }"
                    @click="selectedGroup = 'All'"
                    data-group-id="All"
                >
                    <el-icon class="tree-icon-placeholder"><Folder /></el-icon>
                    <span>全部</span>
                    <span class="count">{{ mods.length }}</span>
                </div>
                <!-- Special Groups for Folder Structure -->
                      <div 
                          class="group-item" 
                          :class="{ active: selectedGroup === 'Root' }"
                            @click="selectedGroup = 'Root'"
                            v-if="mods.some(m => m.group === 'Root')"
                            @dragenter.stop.prevent="onDragEnter"
                            @dragover.stop.prevent="onDragOver"
                            @dragleave.stop="onDragLeave"
                            @drop.stop.prevent="onDrop($event, 'Root')"
                            data-group-id="Root"
                     >
                    <el-icon class="tree-icon-placeholder"><Folder /></el-icon>
                    <span>未分类 (Root)</span>
                    <span class="count">{{ mods.filter(m => m.group === 'Root').length }}</span>
                </div>
                
                <el-tree
                    v-if="groupTree.length > 0"
                    :key="'tree-' + selectedGame"
                    ref="groupTreeRef"
                    :data="groupTree"
                    node-key="id"
                    :props="{ label: 'label', children: 'children' }"
                    :expand-on-click-node="false"
                    :default-expanded-keys="expandedKeys"
                    :current-node-key="selectedGroup"
                    highlight-current
                    @node-click="(data) => selectedGroup = data.id"
                    @node-expand="onGroupExpand"
                    @node-collapse="onGroupCollapse"
                    class="group-tree"
                >
                    <template #default="{ node, data }">
                                <div class="custom-tree-node"
                                    @contextmenu.prevent.stop="showGroupContextMenu($event, data.id)"
                                    @dragenter.stop.prevent="onDragEnter"
                                    @dragover.stop.prevent="onDragOver"
                                    @dragleave.stop="onDragLeave"
                                    @drop.stop.prevent="onDrop($event, data.id)"
                                    :data-group-id="data.id"
                                    :data-parent-id="getGroupParent(data.id)"
                                    :class="{ 'reorder-hover': groupHoverId === data.id }"
                                    @mousedown.stop="onGroupMouseDown($event, data.id)"
                                >
                            <div class="node-content">
                                <img v-if="data.icon" :src="convertFileSrc(data.icon)" class="tree-icon" />
                                <el-icon v-else class="tree-icon-placeholder"><Folder /></el-icon>
                                <span class="node-label" :title="node.label">{{ node.label }}</span>
                            </div>
                            <span class="count" v-if="data.count > 0">{{ data.count }}</span>
                        </div>
                    </template>
                </el-tree>
            </div>
            <!-- Resizer Handle -->
            <div class="sidebar-resizer" @mousedown="startResize"></div>
        </div>

        <!-- Mod Grid -->
        <div class="mod-grid-container" v-loading="loading">
            <div class="manual-sort-hint glass-panel">
                拖拽卡片调整顺序，结果按游戏与分组记忆；也可拖到左侧分类快速归类。
            </div>
            <div v-if="filteredMods.length === 0" class="empty-state">
                <el-empty :description="searchQuery ? '没有找到匹配的 Mod' : '这个游戏还没有 Mod，拖拽压缩包到这里安装！'" >
                    <el-button type="primary" plain @click="openGameFolder">打开文件夹</el-button>
                </el-empty>
            </div>
            
            <div v-else class="mod-grid">
                <div 
                    v-for="mod in filteredMods" 
                    :key="mod.id" 
                    class="mod-card glass-panel"
                    :class="{ 'is-disabled': !mod.enabled, 'reorder-hover': dragOverId === mod.id }"
                    @contextmenu.prevent.stop="showModContextMenu($event, mod)"
                    :draggable="false"
                    @mousedown="onCardMouseDownWrapper($event, mod)"
                    :data-mod-id="mod.id"
                >
                    <!-- Preview Image -->
                    <div class="card-preview">
                        <div v-if="getPreviewUrl(mod)" class="image-wrapper">
                             <el-image 
                                :src="getPreviewUrl(mod)" 
                                fit="cover" 
                                loading="lazy"
                                style="width: 100%; height: 100%;"
                             >
                                <template #error>
                                    <div class="image-placeholder"><el-icon><Picture /></el-icon></div>
                                </template>
                             </el-image>
                        </div>
                        <div v-else class="image-placeholder">
                            <span class="char-avatar">{{ mod.group === 'Root' ? mod.name.charAt(0) : mod.group.charAt(0) }}</span>
                        </div>

                        <!-- Hover Actions -->
                        <div class="card-overlay">
                            <el-button size="small" circle @click.stop="openModFolder(mod.path)" :icon="Folder" title="打开文件夹" />
                        </div>
                    </div>

                    <!-- Info -->
                    <div class="card-info">
                        <div class="header-row">
                            <div class="text-content">
                                <div class="mod-name" :title="mod.name">{{ mod.name }}</div>
                                <div class="mod-group">
                                    <template v-if="mod.group !== 'Root'">
                                        <img v-if="getGroupIcon(mod.group)" :src="convertFileSrc(getGroupIcon(mod.group)!)" class="mini-group-icon" />
                                        <span>{{ mod.group.split('/').pop() }}</span>
                                    </template>
                                    <span v-else>未分类</span>
                                </div>
                            </div>
                            <el-switch 
                                :model-value="mod.enabled"
                                @change="toggleMod(mod)"
                                inline-prompt
                                active-text="ON"
                                inactive-text="OFF"
                                style="--el-switch-on-color: #13ce66; --el-switch-off-color: #ff4949;"
                            />
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <!-- Install Dialog -->
    <el-dialog v-model="showInstallDialog" title="安装 Mod" width="500px" align-center custom-class="glass-dialog">
        <el-form label-width="100px" :model="installForm">
            <el-form-item label="Mod 名称">
                <el-input v-model="installForm.modName" placeholder="建议使用英文" />
            </el-form-item>
            <el-form-item label="分组/角色">
                 <el-autocomplete
                    v-model="installForm.targetGroup"
                    :fetch-suggestions="(qs, cb) => cb(groups.filter((g: any) => g.id !== 'All' && g.id.toLowerCase().includes(qs.toLowerCase())).map((x: any) => ({ value: x.id })))"
                    placeholder="输入角色名或路径(如 A/B)"
                    style="width: 100%"
                >
                    <template #default="{ item }">
                         <span>{{ item.value }}</span>
                    </template>
                </el-autocomplete>
            </el-form-item>
            
            <el-divider>文件预览</el-divider>
            <div class="preview-info" v-if="installPreview">
                <p><strong>格式:</strong> {{ installPreview.format.toUpperCase() }}</p>
                <p><strong>包含文件数:</strong> {{ installPreview.file_count }}</p>
                <p><strong>根目录文件夹:</strong> {{ installPreview.root_dirs.join(', ') || '无 (直接包含文件)' }}</p>
                <p v-if="installPreview.has_ini" style="color: #67c23a"><el-icon><Refresh /></el-icon> 检测到 .ini 文件 (这是一个有效的 Mod)</p>
                <p v-else style="color: #e6a23c">未检测到 .ini 文件，可能是素材包？</p>
            </div>
            <!-- Password prompt if needed in future 
            <el-form-item label="解压密码" v-if="needed">
                 <el-input v-model="installForm.password" />
            </el-form-item> 
            -->
        </el-form>
        <template #footer>
            <span class="dialog-footer">
                <el-button @click="showInstallDialog = false">取消</el-button>
                <el-button type="primary" @click="confirmInstall" :loading="isInstalling">
                    确认安装
                </el-button>
            </span>
        </template>
    </el-dialog>

    <!-- Sub Group Dialog -->
    <el-dialog v-model="subGroupDialog.visible" title="新建子分类" width="420px" align-center custom-class="glass-dialog">
        <el-form label-width="90px">
            <el-form-item label="名称">
                <el-input v-model="subGroupDialog.name" placeholder="请输入子分类名称" />
            </el-form-item>
            <el-form-item label="图标（可选）">
                <div class="subgroup-icon-row">
                    <el-input v-model="subGroupDialog.icon" placeholder="未选择" readonly />
                    <el-button type="primary" plain @click="pickSubGroupIcon">选择图标</el-button>
                </div>
            </el-form-item>
        </el-form>
        <template #footer>
            <span class="dialog-footer">
                <el-button @click="subGroupDialog.visible = false">取消</el-button>
                <el-button type="primary" @click="confirmSubGroup">确认</el-button>
            </span>
        </template>
    </el-dialog>

    <!-- Custom Context Menu -->
    <div 
        v-if="contextMenu.visible"
        class="custom-context-menu glass-panel"
        :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
        @click.stop
    >
        <div v-if="contextMenu.type === 'mod'" class="menu-content">
            <div class="menu-header">{{ contextMenu.target?.name }}</div>
            <div class="menu-divider"></div>
            <div class="menu-item has-submenu">
                <el-icon><FolderAdd /></el-icon>
                <span>移动到...</span>
                <el-icon class="arrow-right"><ArrowRight /></el-icon>
                
                <div class="submenu glass-panel">
                    <div class="menu-item" @click="closeContextMenu(); moveModToGroup(contextMenu.target, 'Root')">
                        <span>未分类 (Root)</span>
                    </div>
                    <div 
                        v-for="group in groups.filter((g: any) => g.id !== 'All' && g.id !== 'Root')" 
                        :key="group.id"
                        class="menu-item"
                        @click="closeContextMenu(); moveModToGroup(contextMenu.target, group.id)"
                    >
                        <span>{{ group.id }}</span>
                    </div>
                    <div class="menu-divider"></div>
                     <div class="menu-item" @click="closeContextMenu(); createNewGroup()">
                        <el-icon><Plus /></el-icon>
                        <span>新建分类...</span>
                    </div>
                </div>
            </div>
            <div class="menu-divider"></div>
            <div class="menu-item" @click="closeContextMenu(); deleteMod(contextMenu.target)" style="color: #ff4949">
                <el-icon><Delete /></el-icon>
                <span>删除</span>
            </div>
        </div>

         <div v-if="contextMenu.type === 'group'" class="menu-content">
            <div class="menu-header">{{ contextMenu.target.split('/').pop() }}</div>
            <div class="menu-divider"></div>
            <div class="menu-item" @click="closeContextMenu(); openModGroupFolder(contextMenu.target)">
                <el-icon><Folder /></el-icon>
                <span>打开文件夹</span>
            </div>
            <div class="menu-item" @click="closeContextMenu(); openSubGroupDialog(contextMenu.target)">
                <el-icon><Plus /></el-icon>
                <span>新建子分类...</span>
            </div>
            <div class="menu-item" @click="closeContextMenu(); setGroupIcon(contextMenu.target)">
                <el-icon><Picture /></el-icon>
                <span>设置图标</span>
            </div>
            <div class="menu-item" @click="closeContextMenu(); renameGroup(contextMenu.target)">
                <el-icon><Edit /></el-icon>
                <span>重命名</span>
            </div>
             <div class="menu-item" @click="closeContextMenu(); deleteGroup(contextMenu.target)" style="color: #ff4949">
                <el-icon><Delete /></el-icon>
                <span>删除</span>
            </div>
        </div>
    </div>
  </div>
</template>

<style scoped>
.page-container.mod-manager {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
    -webkit-app-region: no-drag; /* Ensure content is not treated as window drag area */
}

/* Glass Panel Utility */
.glass-panel {
    background: rgba(30, 30, 35, 0.6);
    backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.2);
}

/* Toolbar */
.toolbar {
    padding: 12px 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 10;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.left-tools, .right-tools {
    display: flex;
    align-items: center;
    gap: 12px;
}

.divider-vertical {
    width: 1px;
    height: 24px;
    background: rgba(255, 255, 255, 0.2);
    margin: 0 8px;
}

/* Main Layout */
.main-content {
    flex: 1;
    display: flex;
    overflow: hidden;
}

/* Sidebar */
.sidebar {
    /* width: 220px; Removed fixed width, handled by inline style */
    flex-shrink: 0;
    overflow-y: auto;
    border-right: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(20, 20, 25, 0.25); /* More transparent */
    position: relative; /* For resizer positioning */
}

.group-list {
    background: rgba(20, 20, 25, 0.35);
    border-radius: 8px;
}

.sidebar-resizer {
    position: absolute;
    top: 0;
    right: 0;
    width: 4px; /* Interaction area */
    height: 100%;
    cursor: col-resize;
    background: transparent;
    z-index: 10;
    transition: background 0.2s;
}

.sidebar-resizer:hover {
    background: rgba(64, 158, 255, 0.5);
}

.group-list {
    padding: 12px;
}

/* Drag Styling */
.group-item.drag-over,
.custom-tree-node.drag-over {
    background: rgba(64, 158, 255, 0.3) !important;
    border-radius: 4px;
    outline: 1px dashed #409eff;
}

.group-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    margin-bottom: 4px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
    color: #a0a0a0;
}

.group-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
}

.group-item.active {
    background: rgba(64, 158, 255, 0.2);
    color: #409eff;
    font-weight: 500;
}

.group-icon {
    width: 20px;
    height: 20px;
    margin-right: 6px;
    border-radius: 4px;
    overflow: hidden;
    flex-shrink: 0;
}
.icon-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
}

.group-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.count {
    font-size: 12px;
    background: rgba(0, 0, 0, 0.2);
    padding: 2px 6px;
    border-radius: 10px;
}

/* Mod Grid */
.mod-grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    /* Custom Scrollbar */
}

.manual-sort-hint {
    padding: 10px 12px;
    margin-bottom: 12px;
    border-radius: 8px;
    font-size: 13px;
    color: #d1e8ff;
    border: 1px dashed rgba(64, 158, 255, 0.5);
}

.subgroup-icon-row {
    display: flex;
    gap: 8px;
}

/* Scrollbar styling for webkit */
.mod-grid-container::-webkit-scrollbar,
.sidebar::-webkit-scrollbar {
    width: 8px;
}
.mod-grid-container::-webkit-scrollbar-track,
.sidebar::-webkit-scrollbar-track {
    background: transparent;
}
.mod-grid-container::-webkit-scrollbar-thumb,
.sidebar::-webkit-scrollbar-thumb {
    background-color: rgba(255, 255, 255, 0.2);
    border-radius: 4px;
}

.mod-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 20px;
}

.mod-card {
    border-radius: 12px;
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
    display: flex;
    flex-direction: column;
    height: 260px;
    background: rgba(30, 30, 35, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    position: relative;
    user-select: none; /* Prevent text selection during drag */
    cursor: grab; /* Indicate draggable */
    z-index: 1;
}

.mod-card[draggable="true"] { cursor: move; }
.mod-card[draggable="false"] { cursor: grab; }

.mod-card.reorder-hover {
    outline: 2px dashed #409eff;
    border-color: rgba(64, 158, 255, 0.4);
}

.mod-card:active {
    cursor: grabbing;
}

.mod-card:hover {
    transform: translateY(-6px);
    box-shadow: 0 12px 24px rgba(0, 0, 0, 0.5);
    border-color: rgba(255, 255, 255, 0.2);
    z-index: 2;
}

/* Disabled State Visuals */
.mod-card.is-disabled {
    opacity: 0.85;
}
.mod-card.is-disabled .image-wrapper {
    filter: grayscale(1) contrast(0.8) brightness(0.8);
    transition: filter 0.3s;
}
.mod-card.is-disabled:hover .image-wrapper {
    filter: grayscale(0.5);
}

.card-preview {
    flex: 1;
    position: relative;
    background: #000;
    overflow: hidden;
}

.image-wrapper {
    width: 100%;
    height: 100%;
    transition: transform 0.5s ease;
}
.mod-card:hover .image-wrapper {
    transform: scale(1.05);
}

/* Prevent image dragging interfering with card dragging */
:deep(.mod-card img) {
    -webkit-user-drag: none;
    pointer-events: none;
}

.image-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #1e1e24, #141417);
    color: rgba(255, 255, 255, 0.2);
    font-size: 48px;
    font-weight: 800;
}
.preview-info {
    font-size: 13px;
    color: #ccc;
    background: rgba(0,0,0,0.2);
    padding: 10px;
    border-radius: 4px;
}
.preview-info p {
    margin: 4px 0;
}

.char-avatar {
    text-transform: uppercase;
}

/* Hover Action Overlay */
.card-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.2s;
    pointer-events: none; /* Let clicks pass through to draggable card */
}

/* Allow interaction with buttons inside overlay */
.card-overlay .el-button {
    pointer-events: auto;
}

.mod-card:hover .card-overlay {
    opacity: 1;
}

/* Footer Info Area */
.card-info {
    padding: 12px 14px;
    background: rgba(18, 18, 20, 0.95);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    height: auto;
    min-height: 64px;
    display: flex;
    flex-direction: column;
    justify-content: center;
}

.header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
}

.text-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.mod-name {
    font-weight: 600;
    color: #f0f0f0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 14px;
    letter-spacing: 0.3px;
}

.mod-group {
    font-size: 11px;
    color: #666;
    display: flex;
    align-items: center;
    gap: 4px;
}
/*.mod-group::before {
    content: '';
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: #666;
}*/
.mini-group-icon {
    width: 14px;
    height: 14px;
    border-radius: 2px;
    object-fit: cover;
}

/* Active dot color if mod is enabled? Could be cool */
/*
.mod-card:not(.is-disabled) .mod-group::before {
    background-color: #67C23A;
    box-shadow: 0 0 6px rgba(103, 194, 58, 0.5);
}
*/


/* Switch styling tweak */
:deep(.el-switch__core) {
    background-color: rgba(255,255,255,0.1);
    border-color: transparent;
}

/* Tree Styles */
.group-tree {
    background: transparent; 
    color: #cfcfcf;
}
:deep(.el-tree-node__content) {
    height: 36px;
    border-radius: 4px;
    margin-bottom: 2px;
}
:deep(.el-tree-node__content:hover) {
    background-color: rgba(255, 255, 255, 0.08) !important;
}
:deep(.el-tree--highlight-current .el-tree-node.is-current > .el-tree-node__content) {
    background-color: rgba(64, 158, 255, 0.15) !important;
    color: #409eff;
}
:deep(.el-tree-node__expand-icon) {
    color: rgba(255, 255, 255, 0.4);
}
:deep(.el-tree-node__expand-icon.is-leaf) {
    color: transparent;
}

.custom-tree-node {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding-right: 8px;
    overflow: hidden;
}

.custom-tree-node.reorder-hover {
    background: rgba(64, 158, 255, 0.15);
    border-radius: 4px;
    outline: 1px dashed #409eff;
}

.node-content {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
}

.node-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 13px;
}

.tree-icon {
    width: 20px;
    height: 20px;
    object-fit: cover;
    border-radius: 4px;
}

.tree-icon-placeholder {
    font-size: 16px;
    color: #888;
}
:deep(.el-switch.is-checked .el-switch__core) {
    background-color: #67C23A;
}

/* Context Menu */
.custom-context-menu {
    position: fixed;
    z-index: 9999;
    background: rgba(30, 30, 30, 0.95);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 4px 0;
    min-width: 160px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
}

.menu-header {
    padding: 8px 16px;
    font-size: 0.85em;
    color: rgba(255, 255, 255, 0.5);
    font-weight: 600;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    margin-bottom: 4px;
}

.menu-item {
    padding: 8px 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: background 0.2s;
    color: #eee;
    font-size: 0.9em;
    position: relative;
}

.menu-item:hover {
    background: rgba(255, 255, 255, 0.1);
}

.menu-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.1);
    margin: 4px 0;
}

.has-submenu .submenu {
    display: none;
    position: absolute;
    left: 100%;
    top: 0;
    margin-left: 4px;
    /* Reuse base styles */
    background: rgba(30, 30, 30, 0.95);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 4px 0;
    min-width: 160px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
}

.has-submenu:hover .submenu {
    display: block;
}

.arrow-right {
    margin-left: auto;
    font-size: 0.8em;
    opacity: 0.7;
}

.sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px 8px 20px;
    margin-bottom: 8px;
}
.sidebar-header .title {
    font-weight: 600;
    color: #fff;
    font-size: 1.1em;
}
</style>
