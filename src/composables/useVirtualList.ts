/**
 * 虚拟列表组合式 API
 * 
 * 用于处理长列表的性能优化，只渲染可视区域内的项目
 * 支持动态行高、滚动监听等功能
 */

import { ref, computed, onMounted, onUnmounted, watch, type Ref } from 'vue';

export interface VirtualListOptions {
  /** 每一行的高度（单位：px），如果列表项高度不一致，建议使用 estimateItemHeight + overscan */
  itemHeight?: number;
  /** 预估的行高（用于初始滚动计算），默认 50 */
  estimateItemHeight?: number;
  /** 缓冲区大小（渲染范围外额外渲染的行数），默认 5 */
  overscan?: number;
  /** 滚动容器的高度，如果不提供则尝试自动获取 */
  containerHeight?: number;
}

export interface VirtualListState {
  /** 已渲染的项目索引范围 */
  startIndex: number;
  endIndex: number;
  /** 已渲染项目列表 */
  visibleItems: any[];
  /** 容器的滚动距离 */
  scrollTop: number;
  /** 总列表高度 */
  totalHeight: number;
}

/**
 * 虚拟列表 Hook
 * 
 * @param items - 完整的列表数据
 * @param options - 配置选项
 * @returns 虚拟列表状态和方法
 * 
 * @example
 * ```ts
 * const { visibleItems, state, containerRef } = useVirtualList(
 *   items,
 *   { itemHeight: 50, overscan: 10 }
 * );
 * ```
 */
export function useVirtualList<T>(
  items: Ref<T[]> | (() => T[]),
  options: VirtualListOptions = {},
) {
  const {
    estimateItemHeight = 50,
    overscan = 5,
    containerHeight: initialContainerHeight,
  } = options;

  // 状态
  const containerRef = ref<HTMLElement | null>(null);
  const scrollTop = ref(0);
  const containerHeight = ref(initialContainerHeight || 0);

  // 计算出列表的实际项目
  const list = computed(() => {
    if (typeof items === 'function') {
      return items();
    }
    return items.value;
  });

  // 计算总高度
  const totalHeight = computed(() => {
    return list.value.length * estimateItemHeight;
  });

  // 计算可见范围
  const state = computed((): VirtualListState => {
    const actualHeight = containerHeight.value || initialContainerHeight || 500;
    
    // 计算起始索引
    const startIndex = Math.max(
      0,
      Math.floor(scrollTop.value / estimateItemHeight) - overscan,
    );

    // 计算结束索引
    const endIndex = Math.min(
      list.value.length,
      Math.ceil((scrollTop.value + actualHeight) / estimateItemHeight) + overscan,
    );

    return {
      startIndex,
      endIndex,
      visibleItems: list.value.slice(startIndex, endIndex),
      scrollTop: scrollTop.value,
      totalHeight: totalHeight.value,
    };
  });

  // 计算偏移距离（用于变换可见项目的位置）
  const offsetY = computed(() => {
    return state.value.startIndex * estimateItemHeight;
  });

  // 处理滚动事件（rAF 节流：每帧最多更新一次，避免高频滚动触发密集响应式更新）
  let rafId: number | null = null;
  const handleScroll = (e: Event) => {
    const target = e.target as HTMLElement;
    const newScrollTop = target.scrollTop;
    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      scrollTop.value = newScrollTop;
      rafId = null;
    });
  };

  // 自动获取容器高度
  const updateContainerHeight = () => {
    if (!containerRef.value) return;

    // 如果未设置 containerHeight，从 DOM 获取
    if (!initialContainerHeight) {
      containerHeight.value = containerRef.value.clientHeight;
    }
  };

  // 滚动到指定索引
  const scrollToIndex = (index: number, behavior: ScrollBehavior = 'smooth') => {
    if (!containerRef.value) return;
    
    const target = Math.max(0, Math.min(index, list.value.length - 1));
    const scrollPosition = target * estimateItemHeight;
    
    containerRef.value.scrollTo({
      top: scrollPosition,
      behavior,
    });
  };

  // 挂载和大小调整监听
  onMounted(() => {
    updateContainerHeight();
    window.addEventListener('resize', updateContainerHeight);
  });

  onUnmounted(() => {
    window.removeEventListener('resize', updateContainerHeight);
  });

  // 监听列表变化时重置滚动位置
  watch(
    () => list.value.length,
    () => {
      if (scrollTop.value > totalHeight.value) {
        scrollTop.value = Math.max(0, totalHeight.value - containerHeight.value);
      }
    },
  );

  return {
    // DOM 引用
    containerRef,
    
    // 状态
    state,
    scrollTop,
    offsetY,
    totalHeight,
    visibleItems: computed(() => state.value.visibleItems),
    startIndex: computed(() => state.value.startIndex),
    
    // 方法
    handleScroll,
    scrollToIndex,
    updateContainerHeight,
  };
}

/**
 * 动态高度虚拟列表 Hook
 * 
 * 用于处理列表项高度不一致的情况
 * 需要在渲染时测量每个项目的实际高度
 */
export function useDynamicVirtualList<T>(
  items: Ref<T[]> | (() => T[]),
  options: Omit<VirtualListOptions, 'itemHeight'> & {
    estimateItemHeight?: number;
  } = {},
) {
  const {
    estimateItemHeight = 50,
    overscan = 5,
    containerHeight: initialContainerHeight,
  } = options;

  const containerRef = ref<HTMLElement | null>(null);
  const scrollTop = ref(0);
  const containerHeight = ref(initialContainerHeight || 0);

  // 存储每个项目的高度缓存
  const itemHeights = ref<Map<number, number>>(new Map());

  const list = computed(() => {
    if (typeof items === 'function') {
      return items();
    }
    return items.value;
  });

  // 获取累积高度（用于二分查找）
  const getCumulativeHeight = (index: number): number => {
    let height = 0;
    for (let i = 0; i < index; i++) {
      height += itemHeights.value.get(i) || estimateItemHeight;
    }
    return height;
  };

  // 总高度
  const totalHeight = computed(() => {
    return getCumulativeHeight(list.value.length);
  });

  // 根据滚动位置二分查找起始项
  const findStartIndex = (): number => {
    let left = 0;
    let right = list.value.length - 1;
    
    while (left <= right) {
      const mid = Math.floor((left + right) / 2);
      const cumulativeHeight = getCumulativeHeight(mid);
      
      if (cumulativeHeight < scrollTop.value) {
        left = mid + 1;
      } else {
        right = mid - 1;
      }
    }
    
    return Math.max(0, left - overscan);
  };

  const state = computed((): VirtualListState => {
    const actualHeight = containerHeight.value || initialContainerHeight || 500;
    const startIndex = findStartIndex();
    
    let accHeight = getCumulativeHeight(startIndex);
    let endIndex = startIndex;

    while (accHeight < scrollTop.value + actualHeight && endIndex < list.value.length) {
      accHeight += itemHeights.value.get(endIndex) || estimateItemHeight;
      endIndex++;
    }

    endIndex = Math.min(endIndex + overscan, list.value.length);

    return {
      startIndex,
      endIndex,
      visibleItems: list.value.slice(startIndex, endIndex),
      scrollTop: scrollTop.value,
      totalHeight: totalHeight.value,
    };
  });

  const offsetY = computed(() => {
    return getCumulativeHeight(state.value.startIndex);
  });

  let rafId: number | null = null;
  const handleScroll = (e: Event) => {
    const target = e.target as HTMLElement;
    const newScrollTop = target.scrollTop;
    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      scrollTop.value = newScrollTop;
      rafId = null;
    });
  };

  const updateItemHeight = (index: number, height: number) => {
    itemHeights.value.set(index, height);
  };

  const updateContainerHeight = () => {
    if (!containerRef.value && !initialContainerHeight) return;
    if (!initialContainerHeight) {
      containerHeight.value = containerRef.value?.clientHeight || 500;
    }
  };

  const scrollToIndex = (index: number, behavior: ScrollBehavior = 'smooth') => {
    if (!containerRef.value) return;
    
    const target = Math.max(0, Math.min(index, list.value.length - 1));
    const scrollPosition = getCumulativeHeight(target);
    
    containerRef.value.scrollTo({
      top: scrollPosition,
      behavior,
    });
  };

  onMounted(() => {
    updateContainerHeight();
    window.addEventListener('resize', updateContainerHeight);
  });

  onUnmounted(() => {
    window.removeEventListener('resize', updateContainerHeight);
  });

  watch(
    () => list.value.length,
    (newLen, oldLen) => {
      // 清理超出范围的缓存
      if (newLen < oldLen) {
        for (let i = newLen; i < oldLen; i++) {
          itemHeights.value.delete(i);
        }
      }
      
      if (scrollTop.value > totalHeight.value) {
        scrollTop.value = Math.max(0, totalHeight.value - containerHeight.value);
      }
    },
  );

  return {
    containerRef,
    state,
    scrollTop,
    offsetY,
    totalHeight,
    visibleItems: computed(() => state.value.visibleItems),
    startIndex: computed(() => state.value.startIndex),
    
    handleScroll,
    updateItemHeight,
    scrollToIndex,
    updateContainerHeight,
  };
}
