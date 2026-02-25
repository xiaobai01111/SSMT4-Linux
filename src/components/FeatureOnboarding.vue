<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
  finishFeatureOnboarding,
  onboardingStepIndex,
  onboardingVisible,
  skipFeatureOnboarding,
} from '../store';

interface GuideStep {
  id: string;
  title: string;
  description: string;
  hint: string;
  routePath: string;
  routeQuery?: Record<string, string>;
  routeWithTimestamp?: boolean;
  selector: string;
  homeAction?: HomeAction;
}

type BubblePlacement = 'top' | 'bottom' | 'left' | 'right' | 'center';
type RuntimeFocusTarget = 'all' | 'wine_version' | 'dxvk' | 'vkd3d';
type OnboardingSettingsTab = 'info' | 'game' | 'runtime' | 'system';
type HomeAction =
  | { type: 'open_download_modal' }
  | { type: 'open_game_settings'; tab: OnboardingSettingsTab; runtimeFocus?: RuntimeFocusTarget }
  | { type: 'close_modals' };

const steps: GuideStep[] = [
  {
    id: 'home-start',
    title: '主页启动区',
    description: '这里是主启动按钮。已安装主程序时可直接启动；未配置时会显示“下载游戏”。',
    hint: '先确认当前选中的游戏，再点击启动按钮。',
    routePath: '/',
    selector: '[data-onboarding="home-start-button"]',
    homeAction: { type: 'close_modals' },
  },
  {
    id: 'home-download',
    title: '下载 / 安装游戏',
    description: '下载窗口重点看三项：状态卡（安装/更新）、安装目录、主操作按钮（下载/校验/防护）。',
    hint: '顺序建议：先确认状态与目录，再开始下载；安装后可直接做一次校验。',
    routePath: '/',
    selector: '[data-onboarding="download-main-actions"], [data-onboarding="download-install-dir"], [data-onboarding="download-state-card"]',
    homeAction: { type: 'open_download_modal' },
  },
  {
    id: 'home-settings',
    title: '主页快速设置',
    description: '右侧齿轮菜单可快速进入“游戏设置”和“下载/防护管理”。',
    hint: '遇到启动异常时，优先从这里进入运行环境检查。',
    routePath: '/',
    selector: '[data-onboarding="home-settings-button"]',
    homeAction: { type: 'close_modals' },
  },
  {
    id: 'library',
    title: '游戏库',
    description: '游戏库用于导入配置、搜索游戏、管理侧边栏显示。',
    hint: '可在游戏项右键进行显示/删除等管理操作。',
    routePath: '/games',
    selector: '[data-onboarding="library-toolbar"], [data-onboarding="library-root"]',
  },
  {
    id: 'game-settings-info',
    title: '游戏信息',
    description: '这里维护配置名称、显示名、版本识别、游戏预设和图标/背景资源，是识别与展示的核心入口。',
    hint: '先把游戏预设与显示名确认正确，再处理图标和背景。',
    routePath: '/',
    selector: '[data-onboarding="game-settings-info-profile"], [data-onboarding="game-settings-tab-info"]',
    homeAction: { type: 'open_game_settings', tab: 'info' },
  },
  {
    id: 'game-settings-game',
    title: '游戏选项',
    description: '这里是启动链路关键：主程序路径、启动参数、工作目录、Prefix 状态，HoYoverse 还会显示 Jadeite 状态。',
    hint: '主程序路径是最关键项，路径不对会导致启动失败或行为异常。',
    routePath: '/',
    selector: '[data-onboarding="game-settings-game-exe"], [data-onboarding="game-settings-tab-game"]',
    homeAction: { type: 'open_game_settings', tab: 'game' },
  },
  {
    id: 'game-settings-runtime',
    title: '运行环境',
    description: '运行环境页签要看三层：Wine/Proton 版本选择、DXVK 状态、VKD3D 状态，并在底部保存。',
    hint: '出现黑屏/秒退时，优先检查这三层是否完整。',
    routePath: '/',
    selector: '[data-onboarding="game-settings-runtime-wine"], [data-onboarding="game-settings-runtime-dxvk"], [data-onboarding="game-settings-tab-runtime"]',
    homeAction: { type: 'open_game_settings', tab: 'runtime', runtimeFocus: 'wine_version' },
  },
  {
    id: 'game-settings-runtime-vkd3d',
    title: '运行环境 - VKD3D',
    description: 'VKD3D 用于 D3D12 转译，默认不强制。需要 D3D12 时再启用并选择版本应用。',
    hint: 'D3D11 游戏通常优先 DXVK；D3D12 相关问题再看 VKD3D。',
    routePath: '/',
    selector: '[data-onboarding="game-settings-runtime-vkd3d"], [data-onboarding="game-settings-tab-runtime"]',
    homeAction: { type: 'open_game_settings', tab: 'runtime', runtimeFocus: 'vkd3d' },
  },
  {
    id: 'game-settings-system',
    title: '系统选项',
    description: '系统选项用于调整显卡选择、游戏语言、沙盒策略，属于兼容性与稳定性调优层。',
    hint: '双显卡设备可优先在这里固定 GPU，再观察性能与稳定性变化。',
    routePath: '/',
    selector: '[data-onboarding="game-settings-system-gpu"], [data-onboarding="game-settings-tab-system"]',
    homeAction: { type: 'open_game_settings', tab: 'system' },
  },
  {
    id: 'settings-menu',
    title: '设置导航',
    description: '左侧是全局设置导航，覆盖基础设置、页面显示、版本检查、资源更新以及运行组件管理。',
    hint: '先完成基础设置，再进入运行组件页面补齐 Proton / DXVK / VKD3D。',
    routePath: '/settings',
    routeQuery: { menu: 'basic' },
    selector: '[data-onboarding="settings-menu"]',
  },
  {
    id: 'settings-basic',
    title: '基础设置',
    description: '基础设置包含语言、数据目录、缓存目录、下载源策略和日志窗口入口。',
    hint: '优先确认 dataDir 与 cacheDir，避免后续下载与前缀路径混乱。',
    routePath: '/settings',
    routeQuery: { menu: 'basic' },
    selector: '[data-onboarding="settings-basic-panel"]',
  },
  {
    id: 'settings-display',
    title: '页面显示设置',
    description: '这里控制顶部导航里的“常用网址 / 使用文档”显示开关。',
    hint: '按你的使用习惯打开需要的入口，减少主页干扰。',
    routePath: '/settings',
    routeQuery: { menu: 'display' },
    selector: '[data-onboarding="settings-display-panel"]',
  },
  {
    id: 'settings-version',
    title: '版本检查',
    description: '版本检查读取本地与最新版本信息，并展示更新日志。',
    hint: '遇到行为差异时，先确认你和文档/他人反馈是否在同一版本。',
    routePath: '/settings',
    routeQuery: { menu: 'version' },
    selector: '[data-onboarding="settings-version-panel"]',
  },
  {
    id: 'settings-resource',
    title: '资源更新',
    description: '资源更新负责同步 Data-parameters，影响游戏预设、下载参数与识别规则。',
    hint: '出现“支持列表不一致/识别异常”时，先执行一次资源更新。',
    routePath: '/settings',
    routeQuery: { menu: 'resource' },
    selector: '[data-onboarding="settings-resource-panel"]',
  },
  {
    id: 'settings-proton',
    title: 'Proton 管理',
    description: '这里管理系统可用的 Wine/Proton 版本，支持刷新本地与远程下载。',
    hint: '若提示“缺少 Proton”，请先在此下载至少一个可用版本。',
    routePath: '/settings',
    routeQuery: { menu: 'proton', guide: '1' },
    routeWithTimestamp: true,
    selector: '[data-onboarding="settings-proton-panel"]',
  },
  {
    id: 'settings-dxvk',
    title: 'DXVK 管理',
    description: '这里下载和维护 DXVK 版本，供游戏 Prefix 安装/切换使用。',
    hint: '若提示“缺少 DXVK”，请先在此下载版本，再到游戏设置应用。',
    routePath: '/settings',
    routeQuery: { menu: 'dxvk', guide: '1' },
    routeWithTimestamp: true,
    selector: '[data-onboarding="settings-dxvk-panel"]',
  },
  {
    id: 'settings-vkd3d',
    title: 'VKD3D 管理',
    description: '这里管理 VKD3D-Proton 下载与缓存，用于 D3D12 转译链路。',
    hint: '如果系统里没有 VKD3D，本页先下载，再回游戏“运行环境”应用版本。',
    routePath: '/settings',
    routeQuery: { menu: 'vkd3d' },
    selector: '[data-onboarding="settings-vkd3d-panel"]',
  },
];

const router = useRouter();
const route = useRoute();
const bubbleRef = ref<HTMLElement | null>(null);
const activeSelector = ref('');
const navigating = ref(false);
const stepBusy = ref(false);

const highlight = reactive({
  visible: false,
  top: 0,
  left: 0,
  width: 0,
  height: 0,
});

const bubble = reactive({
  visible: false,
  top: 0,
  left: 0,
  placement: 'bottom' as BubblePlacement,
  arrowOffset: 24,
});

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const stepIndex = computed(() => {
  if (!Number.isFinite(onboardingStepIndex.value)) return 0;
  const normalized = Math.floor(onboardingStepIndex.value);
  return Math.max(0, Math.min(normalized, steps.length - 1));
});

const currentStep = computed(() => steps[stepIndex.value]);
const isLastStep = computed(() => stepIndex.value >= steps.length - 1);

const clearGuideVisuals = () => {
  highlight.visible = false;
  bubble.visible = false;
};

const isElementVisible = (el: HTMLElement) => {
  const style = window.getComputedStyle(el);
  if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {
    return false;
  }
  const rect = el.getBoundingClientRect();
  return rect.width > 2 && rect.height > 2;
};

const parseSelectors = (selectorText: string) =>
  selectorText
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0);

const findTargetElement = (selectorText: string): HTMLElement | null => {
  const selectors = parseSelectors(selectorText);
  const candidates: HTMLElement[] = [];

  for (const selector of selectors) {
    let matched: NodeListOf<HTMLElement> | null = null;
    try {
      matched = document.querySelectorAll<HTMLElement>(selector);
    } catch {
      matched = null;
    }
    if (!matched || matched.length === 0) continue;
    candidates.push(...Array.from(matched));
  }

  for (const candidate of candidates) {
    if (isElementVisible(candidate)) {
      return candidate;
    }
  }

  for (const candidate of candidates) {
    if (candidate) return candidate;
  }

  return null;
};

const applyHighlightForElement = (target: HTMLElement) => {
  const rect = target.getBoundingClientRect();
  if (rect.width <= 1 || rect.height <= 1) {
    highlight.visible = false;
    return false;
  }
  // 优化：根据元素实际大小动态调整 padding，大元素 padding 小一点，小元素 padding 大一点
  const paddingX = Math.max(4, Math.min(12, 14 - rect.width / 50));
  const paddingY = Math.max(4, Math.min(12, 14 - rect.height / 50));
  
  highlight.top = Math.max(0, rect.top - paddingY);
  highlight.left = Math.max(0, rect.left - paddingX);
  highlight.width = Math.min(window.innerWidth - highlight.left, rect.width + paddingX * 2);
  highlight.height = Math.min(window.innerHeight - highlight.top, rect.height + paddingY * 2);
  highlight.visible = true;
  return true;
};

const updateBubblePosition = async () => {
  if (!onboardingVisible.value) {
    bubble.visible = false;
    return;
  }

  await nextTick();
  const bubbleEl = bubbleRef.value;
  if (!bubbleEl) return;

  const viewportPadding = 12;
  const gap = 14;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const bubbleW = Math.min(bubbleEl.offsetWidth || 360, Math.max(220, vw - viewportPadding * 2));
  const bubbleH = Math.min(bubbleEl.offsetHeight || 240, Math.max(140, vh - viewportPadding * 2));

  if (!highlight.visible) {
    bubble.placement = 'center';
    bubble.left = clamp((vw - bubbleW) / 2, viewportPadding, vw - bubbleW - viewportPadding);
    bubble.top = clamp(vh - bubbleH - 20, viewportPadding, vh - bubbleH - viewportPadding);
    bubble.arrowOffset = 0;
    bubble.visible = true;
    return;
  }

  const tx = highlight.left;
  const ty = highlight.top;
  const tw = highlight.width;
  const th = highlight.height;
  const targetCenterX = tx + tw / 2;
  const targetCenterY = ty + th / 2;

  const spaceTop = ty - viewportPadding;
  const spaceBottom = vh - (ty + th) - viewportPadding;
  const spaceLeft = tx - viewportPadding;
  const spaceRight = vw - (tx + tw) - viewportPadding;

  const canTop = spaceTop >= bubbleH + gap;
  const canBottom = spaceBottom >= bubbleH + gap;
  const canLeft = spaceLeft >= bubbleW + gap;
  const canRight = spaceRight >= bubbleW + gap;

  let placement: BubblePlacement = 'bottom';
  if (canRight) placement = 'right';
  else if (canLeft) placement = 'left';
  else if (canBottom) placement = 'bottom';
  else if (canTop) placement = 'top';
  else {
    const options: Array<[BubblePlacement, number]> = [
      ['right', spaceRight],
      ['left', spaceLeft],
      ['bottom', spaceBottom],
      ['top', spaceTop],
    ];
    options.sort((a, b) => b[1] - a[1]);
    placement = options[0]?.[0] || 'center';
  }

  let top = 0;
  let left = 0;
  let arrowOffset = 20;

  switch (placement) {
    case 'top':
      left = clamp(targetCenterX - bubbleW / 2, viewportPadding, vw - bubbleW - viewportPadding);
      top = clamp(ty - bubbleH - gap, viewportPadding, vh - bubbleH - viewportPadding);
      arrowOffset = clamp(targetCenterX - left, 18, bubbleW - 18);
      break;
    case 'bottom':
      left = clamp(targetCenterX - bubbleW / 2, viewportPadding, vw - bubbleW - viewportPadding);
      top = clamp(ty + th + gap, viewportPadding, vh - bubbleH - viewportPadding);
      arrowOffset = clamp(targetCenterX - left, 18, bubbleW - 18);
      break;
    case 'left':
      left = clamp(tx - bubbleW - gap, viewportPadding, vw - bubbleW - viewportPadding);
      top = clamp(targetCenterY - bubbleH / 2, viewportPadding, vh - bubbleH - viewportPadding);
      arrowOffset = clamp(targetCenterY - top, 18, bubbleH - 18);
      break;
    case 'right':
      left = clamp(tx + tw + gap, viewportPadding, vw - bubbleW - viewportPadding);
      top = clamp(targetCenterY - bubbleH / 2, viewportPadding, vh - bubbleH - viewportPadding);
      arrowOffset = clamp(targetCenterY - top, 18, bubbleH - 18);
      break;
    default:
      placement = 'center';
      left = clamp((vw - bubbleW) / 2, viewportPadding, vw - bubbleW - viewportPadding);
      top = clamp(vh - bubbleH - 20, viewportPadding, vh - bubbleH - viewportPadding);
      arrowOffset = 0;
      break;
  }

  bubble.placement = placement;
  bubble.top = top;
  bubble.left = left;
  bubble.arrowOffset = arrowOffset;
  bubble.visible = true;
};

const updateHighlight = () => {
  if (!onboardingVisible.value || !activeSelector.value) {
    highlight.visible = false;
    void updateBubblePosition();
    return;
  }

  const target = findTargetElement(activeSelector.value);
  if (!target) {
    highlight.visible = false;
    void updateBubblePosition();
    return;
  }

  applyHighlightForElement(target);
  void updateBubblePosition();
};

const ensureRouteForStep = async (step: GuideStep) => {
  const expectedQuery: Record<string, string> = { ...(step.routeQuery || {}) };
  if (step.routeWithTimestamp) {
    expectedQuery.t = String(Date.now());
  }

  const queryMatched = Object.entries(step.routeQuery || {}).every(
    ([k, v]) => String(route.query[k] ?? '') === v,
  );
  const samePath = route.path === step.routePath;
  const shouldNavigate = !samePath || !queryMatched || !!step.routeWithTimestamp;

  if (!shouldNavigate) return;

  navigating.value = true;
  await router.push({
    path: step.routePath,
    query: expectedQuery,
  });
  await nextTick();
  await sleep(90);
  navigating.value = false;
};

const executeHomeAction = async (step: GuideStep) => {
  const action =
    step.homeAction ||
    (step.routePath === '/'
      ? ({ type: 'close_modals' } as HomeAction)
      : null);
  if (!action) return;
  window.dispatchEvent(
    new CustomEvent<HomeAction>('ssmt4-onboarding-action', {
      detail: action,
    }),
  );
  await nextTick();
  await sleep(240);
};

const locateTarget = async (selector: string) => {
  activeSelector.value = selector;
  highlight.visible = false;
  await updateBubblePosition();

  let target = findTargetElement(selector);
  
  if (target) {
    target.scrollIntoView({ block: 'center', inline: 'center', behavior: 'smooth' });
    await sleep(150); // 给滚动一点时间
    target = findTargetElement(selector) || target;
    if (applyHighlightForElement(target)) {
      await updateBubblePosition();
      return;
    }
  }

  // 使用 MutationObserver 替代长轮询等待 DOM 渲染
  return new Promise<void>((resolve) => {
    let timeoutId: number;
    let observer: MutationObserver;
    let resizeObserver: ResizeObserver;
    let isResolved = false;

    const cleanup = () => {
      if (observer) observer.disconnect();
      if (resizeObserver) resizeObserver.disconnect();
      if (timeoutId) clearTimeout(timeoutId);
    };

    const finish = async (foundTarget: HTMLElement | null) => {
      if (isResolved) return;
      isResolved = true;
      cleanup();

      if (foundTarget) {
        foundTarget.scrollIntoView({ block: 'center', inline: 'center', behavior: 'smooth' });
        await sleep(150);
        const latest = findTargetElement(selector) || foundTarget;
        applyHighlightForElement(latest);
      } else {
        highlight.visible = false;
      }
      await updateBubblePosition();
      resolve();
    };

    observer = new MutationObserver(() => {
      const el = findTargetElement(selector);
      if (el) finish(el);
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['style', 'class'],
    });

    if (window.ResizeObserver) {
      resizeObserver = new ResizeObserver(() => {
        const el = findTargetElement(selector);
        if (el) finish(el);
      });
      resizeObserver.observe(document.body);
    }

    // 增加超时控制，最多等待 3.6 秒
    timeoutId = window.setTimeout(() => {
      finish(findTargetElement(selector));
    }, 3600);
  });
};

const syncCurrentStep = async () => {
  if (!onboardingVisible.value) return;
  const step = currentStep.value;
  await ensureRouteForStep(step);
  await executeHomeAction(step);
  await locateTarget(step.selector);
  
  // 如果此时没有高亮，再检查一次，可能是某些动画延迟导致的
  if (!highlight.visible) {
    await sleep(300);
    const retryTarget = findTargetElement(step.selector);
    if (retryTarget && applyHighlightForElement(retryTarget)) {
      await updateBubblePosition();
    } else if (step.homeAction) {
      await executeHomeAction(step);
      await locateTarget(step.selector);
    }
  }
};

const jumpToStep = async (index: number) => {
  if (stepBusy.value) return;
  stepBusy.value = true;
  try {
    onboardingStepIndex.value = Math.max(0, Math.min(Math.floor(index), steps.length - 1));
    await syncCurrentStep();
  } finally {
    stepBusy.value = false;
  }
};

const goPrev = async () => {
  if (stepIndex.value <= 0) return;
  await jumpToStep(stepIndex.value - 1);
};

const goNext = async () => {
  if (isLastStep.value) {
    finishFeatureOnboarding();
    clearGuideVisuals();
    return;
  }
  await jumpToStep(stepIndex.value + 1);
};

const skipGuide = () => {
  if (stepBusy.value) return;
  skipFeatureOnboarding();
  clearGuideVisuals();
};

watch(
  () => onboardingVisible.value,
  async (visible) => {
    if (!visible) {
      clearGuideVisuals();
      return;
    }
    await jumpToStep(stepIndex.value);
  },
);

watch(
  () => route.fullPath,
  async () => {
    if (!onboardingVisible.value) return;
    await nextTick();
    updateHighlight();
  },
);

const onWindowChanged = () => {
  if (!onboardingVisible.value) return;
  updateHighlight();
};

onMounted(() => {
  window.addEventListener('resize', onWindowChanged);
  window.addEventListener('scroll', onWindowChanged, true);
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', onWindowChanged);
  window.removeEventListener('scroll', onWindowChanged, true);
});
</script>

<template>
  <transition name="guide-fade">
    <div v-if="onboardingVisible" class="onboarding-overlay">
      <div v-if="highlight.visible" class="onboarding-focus-ring" :style="{
        top: `${highlight.top}px`,
        left: `${highlight.left}px`,
        width: `${highlight.width}px`,
        height: `${highlight.height}px`,
      }"></div>
      <div v-else class="onboarding-mask"></div>

      <div
        ref="bubbleRef"
        class="onboarding-bubble"
        :class="`placement-${bubble.placement}`"
        :style="{
          top: `${bubble.top}px`,
          left: `${bubble.left}px`,
          '--arrow-offset': `${bubble.arrowOffset}px`,
        }"
      >
        <div class="bubble-top">
          <div class="onboarding-step">功能导览 {{ stepIndex + 1 }} / {{ steps.length }}</div>
        </div>

        <div class="onboarding-title">{{ currentStep.title }}</div>
        <div class="onboarding-desc">{{ currentStep.description }}</div>
        <div class="onboarding-hint">{{ currentStep.hint }}</div>

        <div class="onboarding-actions">
          <el-button size="small" @click="skipGuide">跳过</el-button>
          <el-button size="small" :disabled="stepBusy || stepIndex <= 0" @click="goPrev">上一步</el-button>
          <el-button size="small" type="primary" :loading="navigating || stepBusy" @click="goNext">
            {{ isLastStep ? '完成' : '下一步' }}
          </el-button>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.onboarding-overlay {
  position: fixed;
  inset: 0;
  z-index: 20000;
  pointer-events: none;
}

.onboarding-mask {
  position: fixed;
  inset: 0;
  background: rgba(7, 12, 22, 0.42);
  z-index: 20000;
}

.onboarding-focus-ring {
  position: fixed;
  border-radius: 12px;
  box-shadow:
    0 0 0 9999px rgba(7, 12, 22, 0.42),
    0 0 0 2px rgba(93, 231, 255, 0.95),
    0 0 28px rgba(73, 212, 255, 0.55);
  animation: focusPulse 1.35s ease-in-out infinite;
  pointer-events: none;
  transition: all 0.35s cubic-bezier(0.25, 0.8, 0.25, 1);
  z-index: 20001;
}

.onboarding-bubble {
  position: fixed;
  pointer-events: auto;
  width: min(420px, calc(100vw - 24px));
  background: linear-gradient(
    145deg,
    rgba(15, 28, 40, 0.97) 0%,
    rgba(11, 20, 33, 0.97) 100%
  );
  border: 1px solid rgba(88, 222, 255, 0.52);
  border-radius: 14px;
  color: #ecf8ff;
  box-shadow: 0 18px 44px rgba(0, 0, 0, 0.6);
  padding: 14px 14px 12px;
  transition: all 0.35s cubic-bezier(0.25, 0.8, 0.25, 1);
  z-index: 20002;
}

.onboarding-bubble::after {
  content: '';
  position: absolute;
  width: 12px;
  height: 12px;
  background: rgba(13, 24, 36, 0.98);
  transform: rotate(45deg);
  transition: all 0.35s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.onboarding-bubble.placement-top::after {
  bottom: -7px;
  left: calc(var(--arrow-offset) - 6px);
  border-right: 1px solid rgba(88, 222, 255, 0.52);
  border-bottom: 1px solid rgba(88, 222, 255, 0.52);
}

.onboarding-bubble.placement-bottom::after {
  top: -7px;
  left: calc(var(--arrow-offset) - 6px);
  border-left: 1px solid rgba(88, 222, 255, 0.52);
  border-top: 1px solid rgba(88, 222, 255, 0.52);
}

.onboarding-bubble.placement-left::after {
  right: -7px;
  top: calc(var(--arrow-offset) - 6px);
  border-top: 1px solid rgba(88, 222, 255, 0.52);
  border-right: 1px solid rgba(88, 222, 255, 0.52);
}

.onboarding-bubble.placement-right::after {
  left: -7px;
  top: calc(var(--arrow-offset) - 6px);
  border-left: 1px solid rgba(88, 222, 255, 0.52);
  border-bottom: 1px solid rgba(88, 222, 255, 0.52);
}

.onboarding-bubble.placement-center::after {
  display: none;
}

.bubble-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.onboarding-step {
  display: inline-block;
  padding: 2px 10px;
  border-radius: 999px;
  background: rgba(84, 223, 255, 0.18);
  border: 1px solid rgba(84, 223, 255, 0.34);
  color: #c7f4ff;
  font-size: 12px;
  font-weight: 700;
}

.onboarding-title {
  font-size: 24px;
  font-weight: 800;
  color: #ffffff;
  margin-bottom: 5px;
}

.onboarding-desc {
  font-size: 15px;
  line-height: 1.52;
  color: rgba(236, 247, 255, 0.93);
  margin-bottom: 8px;
}

.onboarding-hint {
  font-size: 13px;
  color: rgba(190, 231, 255, 0.9);
  margin-bottom: 12px;
}

.onboarding-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.guide-fade-enter-active,
.guide-fade-leave-active {
  transition: opacity 0.2s ease;
}

.guide-fade-enter-from,
.guide-fade-leave-to {
  opacity: 0;
}

@keyframes focusPulse {
  0% {
    opacity: 0.9;
  }
  50% {
    opacity: 1;
  }
  100% {
    opacity: 0.9;
  }
}

@media (max-width: 900px) {
  .onboarding-bubble {
    width: min(360px, calc(100vw - 18px));
    padding: 12px;
  }

  .onboarding-title {
    font-size: 20px;
  }

  .onboarding-desc {
    font-size: 14px;
  }
}
</style>
