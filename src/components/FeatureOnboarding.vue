<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import {
  finishFeatureOnboarding,
  onboardingStepIndex,
  onboardingVisible,
  skipFeatureOnboarding,
} from '../store';

interface GuideStep {
  id: string;
  titleKey: string;
  descKey: string;
  hintKey: string;
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

const { t } = useI18n();

const steps: GuideStep[] = [
  {
    id: 'home-start',
    titleKey: 'onboarding.steps.homeStart.title',
    descKey: 'onboarding.steps.homeStart.desc',
    hintKey: 'onboarding.steps.homeStart.hint',
    routePath: '/',
    selector: '[data-onboarding="home-start-button"]',
    homeAction: { type: 'close_modals' },
  },
  {
    id: 'home-download',
    titleKey: 'onboarding.steps.homeDownload.title',
    descKey: 'onboarding.steps.homeDownload.desc',
    hintKey: 'onboarding.steps.homeDownload.hint',
    routePath: '/',
    selector: '[data-onboarding="download-main-actions"], [data-onboarding="download-install-dir"], [data-onboarding="download-state-card"]',
    homeAction: { type: 'open_download_modal' },
  },
  {
    id: 'home-settings',
    titleKey: 'onboarding.steps.homeSettings.title',
    descKey: 'onboarding.steps.homeSettings.desc',
    hintKey: 'onboarding.steps.homeSettings.hint',
    routePath: '/',
    selector: '[data-onboarding="home-settings-button"]',
    homeAction: { type: 'close_modals' },
  },
  {
    id: 'library',
    titleKey: 'onboarding.steps.library.title',
    descKey: 'onboarding.steps.library.desc',
    hintKey: 'onboarding.steps.library.hint',
    routePath: '/games',
    selector: '[data-onboarding="library-toolbar"], [data-onboarding="library-root"]',
  },
  {
    id: 'game-settings-info',
    titleKey: 'onboarding.steps.gameSettingsInfo.title',
    descKey: 'onboarding.steps.gameSettingsInfo.desc',
    hintKey: 'onboarding.steps.gameSettingsInfo.hint',
    routePath: '/',
    selector: '[data-onboarding="game-settings-info-profile"], [data-onboarding="game-settings-tab-info"]',
    homeAction: { type: 'open_game_settings', tab: 'info' },
  },
  {
    id: 'game-settings-game',
    titleKey: 'onboarding.steps.gameSettingsGame.title',
    descKey: 'onboarding.steps.gameSettingsGame.desc',
    hintKey: 'onboarding.steps.gameSettingsGame.hint',
    routePath: '/',
    selector: '[data-onboarding="game-settings-game-exe"], [data-onboarding="game-settings-tab-game"]',
    homeAction: { type: 'open_game_settings', tab: 'game' },
  },
  {
    id: 'game-settings-runtime',
    titleKey: 'onboarding.steps.gameSettingsRuntime.title',
    descKey: 'onboarding.steps.gameSettingsRuntime.desc',
    hintKey: 'onboarding.steps.gameSettingsRuntime.hint',
    routePath: '/',
    selector: '[data-onboarding="game-settings-runtime-wine"], [data-onboarding="game-settings-runtime-dxvk"], [data-onboarding="game-settings-tab-runtime"]',
    homeAction: { type: 'open_game_settings', tab: 'runtime', runtimeFocus: 'wine_version' },
  },
  {
    id: 'game-settings-runtime-vkd3d',
    titleKey: 'onboarding.steps.gameSettingsRuntimeVkd3d.title',
    descKey: 'onboarding.steps.gameSettingsRuntimeVkd3d.desc',
    hintKey: 'onboarding.steps.gameSettingsRuntimeVkd3d.hint',
    routePath: '/',
    selector: '[data-onboarding="game-settings-runtime-vkd3d"], [data-onboarding="game-settings-tab-runtime"]',
    homeAction: { type: 'open_game_settings', tab: 'runtime', runtimeFocus: 'vkd3d' },
  },
  {
    id: 'game-settings-system',
    titleKey: 'onboarding.steps.gameSettingsSystem.title',
    descKey: 'onboarding.steps.gameSettingsSystem.desc',
    hintKey: 'onboarding.steps.gameSettingsSystem.hint',
    routePath: '/',
    selector: '[data-onboarding="game-settings-system-gpu"], [data-onboarding="game-settings-tab-system"]',
    homeAction: { type: 'open_game_settings', tab: 'system' },
  },
  {
    id: 'settings-menu',
    titleKey: 'onboarding.steps.settingsMenu.title',
    descKey: 'onboarding.steps.settingsMenu.desc',
    hintKey: 'onboarding.steps.settingsMenu.hint',
    routePath: '/settings',
    routeQuery: { menu: 'basic' },
    selector: '[data-onboarding="settings-menu"]',
  },
  {
    id: 'settings-basic',
    titleKey: 'onboarding.steps.settingsBasic.title',
    descKey: 'onboarding.steps.settingsBasic.desc',
    hintKey: 'onboarding.steps.settingsBasic.hint',
    routePath: '/settings',
    routeQuery: { menu: 'basic' },
    selector: '[data-onboarding="settings-basic-panel"]',
  },
  {
    id: 'settings-display',
    titleKey: 'onboarding.steps.settingsDisplay.title',
    descKey: 'onboarding.steps.settingsDisplay.desc',
    hintKey: 'onboarding.steps.settingsDisplay.hint',
    routePath: '/settings',
    routeQuery: { menu: 'display' },
    selector: '[data-onboarding="settings-display-panel"]',
  },
  {
    id: 'settings-version',
    titleKey: 'onboarding.steps.settingsVersion.title',
    descKey: 'onboarding.steps.settingsVersion.desc',
    hintKey: 'onboarding.steps.settingsVersion.hint',
    routePath: '/settings',
    routeQuery: { menu: 'version' },
    selector: '[data-onboarding="settings-version-panel"]',
  },
  {
    id: 'settings-resource',
    titleKey: 'onboarding.steps.settingsResource.title',
    descKey: 'onboarding.steps.settingsResource.desc',
    hintKey: 'onboarding.steps.settingsResource.hint',
    routePath: '/settings',
    routeQuery: { menu: 'resource' },
    selector: '[data-onboarding="settings-resource-panel"]',
  },
  {
    id: 'settings-proton',
    titleKey: 'onboarding.steps.settingsProton.title',
    descKey: 'onboarding.steps.settingsProton.desc',
    hintKey: 'onboarding.steps.settingsProton.hint',
    routePath: '/settings',
    routeQuery: { menu: 'proton', guide: '1' },
    routeWithTimestamp: true,
    selector: '[data-onboarding="settings-proton-panel"]',
  },
  {
    id: 'settings-dxvk',
    titleKey: 'onboarding.steps.settingsDxvk.title',
    descKey: 'onboarding.steps.settingsDxvk.desc',
    hintKey: 'onboarding.steps.settingsDxvk.hint',
    routePath: '/settings',
    routeQuery: { menu: 'dxvk', guide: '1' },
    routeWithTimestamp: true,
    selector: '[data-onboarding="settings-dxvk-panel"]',
  },
  {
    id: 'settings-vkd3d',
    titleKey: 'onboarding.steps.settingsVkd3d.title',
    descKey: 'onboarding.steps.settingsVkd3d.desc',
    hintKey: 'onboarding.steps.settingsVkd3d.hint',
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
          <div class="onboarding-step">{{ t('onboarding.progress', { current: stepIndex + 1, total: steps.length }) }}</div>
        </div>

        <div class="onboarding-title">{{ t(currentStep.titleKey) }}</div>
        <div class="onboarding-desc">{{ t(currentStep.descKey) }}</div>
        <div class="onboarding-hint">{{ t(currentStep.hintKey) }}</div>

        <div class="onboarding-actions">
          <el-button size="small" @click="skipGuide">{{ t('onboarding.actions.skip') }}</el-button>
          <el-button size="small" :disabled="stepBusy || stepIndex <= 0" @click="goPrev">{{ t('onboarding.actions.prev') }}</el-button>
          <el-button size="small" type="primary" :loading="navigating || stepBusy" @click="goNext">
            {{ isLastStep ? t('onboarding.actions.finish') : t('onboarding.actions.next') }}
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
