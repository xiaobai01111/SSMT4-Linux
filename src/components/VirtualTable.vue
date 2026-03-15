<script setup lang="ts" generic="T extends Record<string, any>">
import { computed } from 'vue';
import { useVirtualList } from '../composables/useVirtualList';

export interface VirtualTableColumn<T> {
  /** 列 key */
  prop: keyof T;
  /** 列标签 */
  label: string;
  /** 列宽度 */
  width?: string | number;
  /** 最小宽度 */
  minWidth?: string | number;
  /** 是否允许排序 */
  sortable?: boolean;
  /** 渲染函数（可选，用于自定义单元格） */
  render?: (row: T, index: number) => any;
}

export interface VirtualTableProps<T> {
  /** 表格数据 */
  data: T[];
  /** 列配置 */
  columns: VirtualTableColumn<T>[];
  /** 行高（单位：px） */
  rowHeight?: number;
  /** 表格高度 */
  height?: string | number;
  /** 唯一标识符（用于行 key） */
  rowKey?: keyof T | ((row: T, index: number) => string | number);
  /** 是否显示加载状态 */
  loading?: boolean;
  /** 空状态文本 */
  emptyText?: string;
}

const props = withDefaults(defineProps<VirtualTableProps<T>>(), {
  rowHeight: 50,
  height: '100%',
  emptyText: '暂无数据',
});

const emit = defineEmits<{
  (event: 'row-click', row: T, index: number): void;
}>();

const dataRef = computed(() => props.data);

const {
  offsetY,
  startIndex,
  visibleItems,
  totalHeight,
  handleScroll,
} = useVirtualList(dataRef, {
  itemHeight: props.rowHeight,
  overscan: 5,
  containerHeight: typeof props.height === 'number' ? props.height : undefined,
});

// 获取行的唯一 key
const getRowKey = (row: T, index: number): string | number => {
  if (typeof props.rowKey === 'function') {
    return props.rowKey(row, index);
  }
  if (props.rowKey) {
    return row[props.rowKey] ?? index;
  }
  return index;
};

// 计算列宽
const columnWidths = computed(() => {
  const widths: Record<string, string> = {};
  for (const col of props.columns) {
    if (col.width) {
      widths[String(col.prop)] = String(col.width).includes('%') || String(col.width).includes('px')
        ? String(col.width)
        : `${col.width}px`;
    }
  }
  return widths;
});
</script>

<template>
  <div class="virtual-table-wrapper" :style="{ height: typeof height === 'number' ? `${height}px` : height }">
    <!-- 加载中遮罩 -->
    <div v-if="loading" class="table-loading-mask">
      <el-icon class="is-loading">
        <Loading />
      </el-icon>
    </div>

    <!-- 表头 -->
    <div class="virtual-table-header">
      <div class="table-row" :style="{ height: `${rowHeight}px` }">
        <div
          v-for="col in columns"
          :key="String(col.prop)"
          class="table-cell table-header-cell"
          :style="columnWidths[String(col.prop)] ? { width: columnWidths[String(col.prop)] } : { flex: 1 }"
        >
          {{ col.label }}
        </div>
      </div>
    </div>

    <!-- 虚拟表体 -->
    <div
      class="virtual-table-body"
      @scroll="handleScroll"
    >
      <!-- 空状态 -->
      <div v-if="data.length === 0 && !loading" class="table-empty">
        {{ emptyText }}
      </div>

      <!-- 占位符 -->
      <div v-else class="table-spacer" :style="{ height: `${totalHeight}px` }">
        <!-- 虚拟行容器 -->
        <div
          class="table-content"
          :style="{ transform: `translateY(${offsetY}px)` }"
        >
          <div
            v-for="(row, index) in visibleItems"
            :key="getRowKey(row, startIndex + index)"
            class="table-row"
            :style="{ height: `${rowHeight}px`, cursor: 'pointer' }"
            @click="emit('row-click', row, startIndex + index)"
          >
            <div
              v-for="col in columns"
              :key="String(col.prop)"
              class="table-cell"
              :style="columnWidths[String(col.prop)] ? { width: columnWidths[String(col.prop)] } : { flex: 1 }"
            >
              <slot :name="`${String(col.prop)}`" :row="row" :index="startIndex + index">
                {{ col.render ? col.render(row, startIndex + index) : row[col.prop] }}
              </slot>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.virtual-table-wrapper {
  width: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid rgba(0, 240, 255, 0.18);
  background: rgba(0, 8, 14, 0.55);
  border-radius: 10px;
  overflow: hidden;
  position: relative;
}

.table-loading-mask {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 8, 14, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  border-radius: 10px;
}

.table-loading-mask :deep(.is-loading) {
  animation: rotating 2s linear infinite;
  color: #00f0ff;
  font-size: 32px;
}

@keyframes rotating {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.virtual-table-header {
  flex-shrink: 0;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  overflow-y: auto;
  overflow-x: hidden;
}

.virtual-table-body {
  flex: 1;
  overflow: auto;
  position: relative;
}

.table-spacer {
  position: relative;
  width: 100%;
}

.table-content {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  width: 100%;
}

.table-row {
  display: flex;
  align-items: center;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.table-row:hover {
  background: rgba(0, 240, 255, 0.06);
}

.table-cell {
  padding: 12px 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.88);
  font-size: 14px;
  display: flex;
  align-items: center;
}

.table-header-cell {
  font-weight: 600;
  color: rgba(255, 255, 255, 0.72);
  background: rgba(255, 255, 255, 0.03);
  user-select: none;
}

.table-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: rgba(255, 255, 255, 0.45);
  font-size: 14px;
}

/* 滚动条样式 */
.virtual-table-body::-webkit-scrollbar,
.virtual-table-header::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.virtual-table-body::-webkit-scrollbar-track,
.virtual-table-header::-webkit-scrollbar-track {
  background: transparent;
}

.virtual-table-body::-webkit-scrollbar-thumb,
.virtual-table-header::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 3px;
}

.virtual-table-body::-webkit-scrollbar-thumb:hover,
.virtual-table-header::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.25);
}
</style>
