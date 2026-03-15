<script setup lang="ts" generic="T extends Record<string, any>">
import { computed } from 'vue';
import { useVirtualList, type VirtualListOptions } from '../composables/useVirtualList';

export interface VirtualListProps<T> {
  /** 完整的列表数据 */
  items: T[];
  /** 虚拟列表配置选项 */
  options?: VirtualListOptions;
  /** 用于渲染单个项目的插槽数据 */
  itemKey?: keyof T | ((item: T, index: number) => string | number);
}

const props = withDefaults(
  defineProps<VirtualListProps<T>>(),
  {
    options: () => ({ itemHeight: 50, overscan: 5 }),
    itemKey: (_item: T, index: number) => index,
  }
);

const itemsRef = computed(() => props.items);

const {
  containerRef,
  offsetY,
  startIndex,
  visibleItems,
  totalHeight,
  handleScroll,
  scrollToIndex,
} = useVirtualList(itemsRef, props.options);

// 获取每个项目的唯一 key
const getItemKey = (item: T, index: number): string | number => {
  const actualIndex = startIndex.value + index;
  if (typeof props.itemKey === 'function') {
    return props.itemKey(item, actualIndex);
  }
  return item?.[props.itemKey] ?? actualIndex;
};

defineExpose({
  scrollToIndex,
  containerRef,
});
</script>

<template>
  <div
    ref="containerRef"
    class="virtual-list-container"
    @scroll="handleScroll"
  >
    <!-- 占位符，用于保持滚动条大小正确 -->
    <div class="virtual-list-spacer" :style="{ height: totalHeight + 'px' }">
      <!-- 虚拟项目容器，使用 transform 优化性能 -->
      <div
        class="virtual-list-content"
        :style="{ transform: `translateY(${offsetY}px)` }"
      >
        <div
          v-for="(item, index) in visibleItems"
          :key="getItemKey(item, index)"
          class="virtual-list-item"
        >
          <!-- 使用 slot 渲染每个项目 -->
          <slot
            :item="item"
            :index="startIndex + index"
            :items="items"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.virtual-list-container {
  width: 100%;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  position: relative;
}

.virtual-list-spacer {
  position: relative;
  width: 100%;
}

.virtual-list-content {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  width: 100%;
}

.virtual-list-item {
  width: 100%;
}

/* 自定义滚动条样式 */
.virtual-list-container::-webkit-scrollbar {
  width: 8px;
}

.virtual-list-container::-webkit-scrollbar-track {
  background: transparent;
}

.virtual-list-container::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  transition: background 0.3s;
}

.virtual-list-container::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.4);
}
</style>
