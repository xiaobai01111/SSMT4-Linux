<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';

export interface VirtualLogLineProps {
  /** 日志内容（按行分割） */
  content: string;
  /** 预估行高，默认 1.6em = 约 25.6px */
  estimateLineHeight?: number;
  /** 缓冲区大小 */
  overscan?: number;
  /** 是否自动滚动到底部 */
  autoScroll?: boolean;
}

const props = withDefaults(defineProps<VirtualLogLineProps>(), {
  estimateLineHeight: 25.6,
  overscan: 5,
  autoScroll: true,
});

const containerRef = ref<HTMLDivElement | null>(null);
const scrollTop = ref(0);
const containerHeight = ref(0);
const isAtBottom = ref(true);

// 将日志分割成行
const lines = computed(() => {
  if (!props.content) return [];
  return props.content.split('\n');
});

// 计算总高度
const totalHeight = computed(() => {
  return lines.value.length * props.estimateLineHeight;
});

// 计算可见范围
const visibleRange = computed(() => {
  const actualHeight = containerHeight.value || 500;
  const startIndex = Math.max(
    0,
    Math.floor(scrollTop.value / props.estimateLineHeight) - props.overscan,
  );

  const endIndex = Math.min(
    lines.value.length,
    Math.ceil((scrollTop.value + actualHeight) / props.estimateLineHeight) + props.overscan,
  );

  return { startIndex, endIndex };
});

const visibleLines = computed(() => {
  const { startIndex, endIndex } = visibleRange.value;
  return lines.value.slice(startIndex, endIndex).map((line, index) => ({
    index: startIndex + index,
    content: line,
  }));
});

const offsetY = computed(() => {
  return visibleRange.value.startIndex * props.estimateLineHeight;
});

let scrollRafId: number | null = null;
const handleScroll = (e: Event) => {
  const target = e.target as HTMLElement;
  const newScrollTop = target.scrollTop;
  const isBottom = target.scrollHeight - newScrollTop - target.clientHeight < 10;
  if (scrollRafId !== null) return;
  scrollRafId = requestAnimationFrame(() => {
    scrollTop.value = newScrollTop;
    isAtBottom.value = isBottom;
    scrollRafId = null;
  });
};

const scrollToBottom = () => {
  if (!containerRef.value) return;
  containerRef.value.scrollTop = containerRef.value.scrollHeight;
  isAtBottom.value = true;
};

const updateContainerHeight = () => {
  if (!containerRef.value) return;
  containerHeight.value = containerRef.value.clientHeight;
};

// 监听内容变化，自动滚动到底部
let contentChangeTimeout: ReturnType<typeof setTimeout> | null = null;
const handleContentChange = () => {
  if (contentChangeTimeout) clearTimeout(contentChangeTimeout);
  
  if (props.autoScroll) {
    // 延迟滚动，确保 DOM 更新完成
    contentChangeTimeout = setTimeout(() => {
      scrollToBottom();
    }, 0);
  }
};

onMounted(() => {
  updateContainerHeight();
  window.addEventListener('resize', updateContainerHeight);
  
  // 初始滚动到底部
  if (props.autoScroll) {
    setTimeout(scrollToBottom, 0);
  }
});

onUnmounted(() => {
  window.removeEventListener('resize', updateContainerHeight);
  if (contentChangeTimeout) clearTimeout(contentChangeTimeout);
  if (scrollRafId !== null) cancelAnimationFrame(scrollRafId);
});

// 监听内容变化
watch(() => props.content, handleContentChange);

defineExpose({
  containerRef,
  scrollToBottom,
  updateContainerHeight,
  isAtBottom,
});
</script>

<template>
  <div
    ref="containerRef"
    class="virtual-log-viewer"
    @scroll="handleScroll"
  >
    <!-- 占位符 -->
    <div class="log-spacer" :style="{ height: totalHeight + 'px' }">
      <!-- 虚拟日志行容器 -->
      <div
        class="log-content"
        :style="{ transform: `translateY(${offsetY}px)` }"
      >
        <div
          v-for="line in visibleLines"
          :key="line.index"
          class="log-line"
        >
          {{ line.content || ' ' }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.virtual-log-viewer {
  width: 100%;
  height: 100%;
  overflow-y: auto;
  overflow-x: auto;
  position: relative;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 11px;
  line-height: 1.6;
  white-space: pre;
  word-break: normal;
  color: #c8c8c8;
  background: #0d0d0d;
}

.log-spacer {
  position: relative;
  width: 100%;
  min-width: max-content;
}

.log-content {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  width: 100%;
  min-width: max-content;
}

.log-line {
  padding-right: 16px;
  white-space: pre-wrap;
  word-wrap: break-word;
}

/* 滚动条样式 */
.virtual-log-viewer::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.virtual-log-viewer::-webkit-scrollbar-track {
  background: transparent;
}

.virtual-log-viewer::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 3px;
}

.virtual-log-viewer::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.25);
}

.virtual-log-viewer::-webkit-scrollbar-corner {
  background: transparent;
}
</style>
