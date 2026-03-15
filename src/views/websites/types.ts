export type LinkCategory = 'project' | 'games';

export type LinkItem = {
  key: string;
  category: LinkCategory;
  url?: string;
  cnUrl?: string;
  globalUrl?: string;
};
