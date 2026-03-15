export type ModStatusFilter = 'all' | 'enabled' | 'disabled';

export type ModGameSummary = {
  name: string;
  displayName: string;
  fallbackDisplayName: string;
  searchNames: string[];
  iconPath: string;
  migotoSupported: boolean;
  importer: string;
  migotoEnabled: boolean;
  modFolder: string;
  shaderFixesFolder: string;
  modFolderExists: boolean;
  shaderFixesFolderExists: boolean;
  loadError: string;
};
