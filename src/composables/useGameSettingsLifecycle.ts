import { ref } from 'vue';

type AskConfirm = (
  message: string,
  options?: { title?: string; kind?: 'warning' | 'info' | 'error' },
) => Promise<boolean>;
type Translate = (key: string, fallback: string) => string;

interface GameNameChangeOptions {
  newGame: string | null | undefined;
  oldGame: string | null | undefined;
  isModalOpen: boolean;
  hasLoadedConfig: boolean;
  hasUnsavedInfoChanges: boolean;
  saveManagedSections: (targetGameName?: string) => Promise<void>;
  loadAllSections: () => Promise<unknown>;
  revertToGame: (gameName: string) => void;
}

interface CloseRequestOptions {
  hasUnsavedInfoChanges: boolean;
  onClose: () => void;
}

export function useGameSettingsLifecycle(options: {
  askConfirm: AskConfirm;
  tr: Translate;
}) {
  const suppressNextGameSwitch = ref(false);

  const confirmDiscardInfoChanges = async (
    reason: 'close' | 'switch',
  ): Promise<boolean> => {
    const message =
      reason === 'switch'
        ? options.tr(
            'gamesettingsmodal.info.unsavedSwitchPrompt',
            '游戏信息页存在未保存更改，切换游戏将丢失这些修改，确定继续吗？',
          )
        : options.tr(
            'gamesettingsmodal.info.unsavedPrompt',
            '游戏信息页存在未保存更改，确定要关闭吗？',
          );

    return options.askConfirm(message, {
      title: options.tr(
        'gamesettingsmodal.info.unsavedTitle',
        '未保存更改',
      ),
      kind: 'warning',
    });
  };

  const handleGameNameChange = async ({
    newGame,
    oldGame,
    isModalOpen,
    hasLoadedConfig,
    hasUnsavedInfoChanges,
    saveManagedSections,
    loadAllSections,
    revertToGame,
  }: GameNameChangeOptions) => {
    if (!newGame || newGame === oldGame) return;

    if (suppressNextGameSwitch.value) {
      suppressNextGameSwitch.value = false;
      return;
    }

    if (!isModalOpen) return;

    if (oldGame && hasLoadedConfig) {
      if (hasUnsavedInfoChanges) {
        const discard = await confirmDiscardInfoChanges('switch');
        if (!discard) {
          suppressNextGameSwitch.value = true;
          revertToGame(oldGame);
          return;
        }
      }

      await saveManagedSections(oldGame);
    }

    await loadAllSections();
  };

  const requestClose = async ({
    hasUnsavedInfoChanges,
    onClose,
  }: CloseRequestOptions): Promise<boolean> => {
    if (hasUnsavedInfoChanges) {
      const discard = await confirmDiscardInfoChanges('close');
      if (!discard) {
        return false;
      }
    }

    onClose();
    return true;
  };

  return {
    handleGameNameChange,
    requestClose,
  };
}
