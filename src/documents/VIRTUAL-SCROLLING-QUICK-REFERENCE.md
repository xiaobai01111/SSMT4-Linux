# 虚拟滚动快速参考

## 🚀 快速开始

### 场景1：渲染大量日志
```vue
<VirtualLogViewer
  :content="logContent"
  :auto-scroll="true"
  :estimate-line-height="25.6"
  :overscan="5"
/>
```

### 场景2：渲染大量列表项
```vue
<script setup lang="ts">
import { ref } from 'vue';
import VirtualList from '@/components/VirtualList.vue';

const items = ref(/* 数千项数据 */);
</script>

<template>
  <VirtualList :items="items" item-key="id">
    <template #default="{ item }">
      <div>{{ item.name }}</div>
    </template>
  </VirtualList>
</template>
```

### 场景3：在 useVirtualList Hook 中自定义
```typescript
const { visibleItems, offsetY, scrollToIndex } = useVirtualList(
  itemsRef,
  { itemHeight: 50, overscan: 5 }
);

// 手动渲染
const renderedItems = computed(() => 
  visibleItems.value.map((item, idx) => ({
    ...item,
    style: { transform: `translateY(${offsetY.value}px)` }
  }))
);
```

## 📖 API 快速查询

| 组件/Hook | 主要参数 | 返回值 |
|----------|--------|--------|
| `VirtualLogViewer` | `content`, `autoScroll`, `estimateLineHeight` | DOM元素、滚动方法 |
| `VirtualList` | `items`, `itemKey`, `options` | Slot插槽、滚动控制 |
| `useVirtualList()` | `items`, `options` | `visibleItems`, `offsetY`, `scrollToIndex()` |
| `VirtualTable` | `data`, `columns`, `rowHeight` | 虚拟表格渲染 |

## ⚡ 性能优化技巧

```typescript
// 1. 调整缓冲区大小
const { visibleItems } = useVirtualList(items, {
  overscan: 3  // 快速网络，值更小
});

// 2. 预估准确的行高
:estimate-line-height="25.6"  // 11px字体 + 1.6行高

// 3. 使用 Key 避免重渲染
<VirtualList item-key="id" />  // 必须

// 4. 对于动态高度，使用 useDynamicVirtualList
const { updateItemHeight } = useDynamicVirtualList(items);
```

## 🐛 调试技巧

```typescript
// 1. 检查虚拟窗口范围
const { startIndex, endIndex } = state.value;
console.log(`Rendering items ${startIndex} to ${endIndex}`);

// 2. 验证容器高度
console.log('Container height:', containerHeight.value);

// 3. 查看滚动位置
console.log('Scroll position:', scrollTop.value);
```

## ❌ 常见错误

| 错误 | 原因 | 解决方案 |
|-----|------|--------|
| 列表项重叠 | `overscan` 太小或容器高度错误 | 增加 `overscan` 或设置正确的 `containerHeight` |
| 白屏 | 列表为空或 `items` 未绑定 | 检查 `items` 数据绑定 |
| 滚动卡顿 | 渲染函数中有昂贵计算 | 将计算移到外部或使用 `computed` |
| 行高不匹配 | `estimateItemHeight` 与实际高度差异大 | 使用 `useDynamicVirtualList()` |

## 📦 已改造的页面

| 页面 | 组件 | 改进 |
|------|------|------|
| 日志查看器 | `VirtualLogViewer` | 支持数万行日志 |
| 游戏日志 | `VirtualLogViewer` | 流式日志更新 |
| MOD管理 | `el-table` + `max-height` | 数千MOD快速滚动 |

## 🔗 相关文件

- **实现**：`src/composables/useVirtualList.ts`
- **组件**：
  - `src/components/VirtualLogViewer.vue`
  - `src/components/VirtualList.vue`
  - `src/components/VirtualTable.vue`
- **文档**：`src/documents/virtual-scrolling-guide.md`
- **使用**：
  - `src/views/log-viewer/index.vue`
  - `src/views/game-log-viewer/index.vue`
  - `src/views/mods/ModsEntryTable.vue`

---

**更多详情**：查看 `src/documents/virtual-scrolling-guide.md`
