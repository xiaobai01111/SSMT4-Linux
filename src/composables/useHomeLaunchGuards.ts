import type { GameConfig } from '../api';
import type { HomeLaunchGuardPlan } from '../types/homeLaunch';
import {
  checkProtectionRequirement,
  checkRuntimeReadiness,
  probeExecutablePathMismatch,
  resolveWineVersionIdForLaunch,
} from './homeLaunchPolicy';
import {
  buildExecutableGuardSteps,
  buildProtectionGuardSteps,
  buildRuntimeGuardSteps,
  findBlockingStep,
} from './homeLaunchGuardPlan';

export function useHomeLaunchGuards() {
  const planLaunchGuards = async (
    gameName: string,
    gameConfig: GameConfig,
    hasExecutablePath: boolean,
  ): Promise<HomeLaunchGuardPlan> => {
    const steps = buildProtectionGuardSteps(
      await checkProtectionRequirement(gameName, gameConfig),
    );

    if (findBlockingStep(steps) || !hasExecutablePath) {
      return {
        wineVersionId: '',
        steps,
      };
    }

    const wineVersionId = await resolveWineVersionIdForLaunch(gameName, gameConfig);
    const runtimeSteps = buildRuntimeGuardSteps(
      await checkRuntimeReadiness(gameName, gameConfig, wineVersionId),
    );
    steps.push(...runtimeSteps);

    if (!findBlockingStep(runtimeSteps)) {
      steps.push(
        ...buildExecutableGuardSteps(
          await probeExecutablePathMismatch(gameName, gameConfig),
        ),
      );
    }

    return {
      wineVersionId,
      steps,
    };
  };

  return {
    planLaunchGuards,
  };
}
