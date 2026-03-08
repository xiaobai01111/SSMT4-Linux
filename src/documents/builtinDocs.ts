const docModules = import.meta.glob('./content/*.md', {
  eager: true,
  import: 'default',
  query: '?raw',
}) as Record<string, string>;

export type BuiltinDocDefinition = {
  id: string;
  titleKey: string;
  fallbackTitle: string;
  file: string;
};

export const builtinDocCatalog: BuiltinDocDefinition[] = [
  { id: 'home', titleKey: 'documents.items.home', fallbackTitle: 'Home', file: 'Home.md' },
  { id: 'terms', titleKey: 'documents.items.terms', fallbackTitle: '《服务条款》与风险声明', file: '00-服务条款与风险声明.md' },
  { id: 'risk', titleKey: 'documents.items.risk', fallbackTitle: '项目风险与要求', file: '01-项目风险与要求.md' },
  { id: 'download', titleKey: 'documents.items.download', fallbackTitle: '游戏下载与主程序配置', file: '02-游戏下载与主程序配置.md' },
  { id: 'downloadMechanism', titleKey: 'documents.items.downloadMechanism', fallbackTitle: '下载、校验、修复机制与厂商差异', file: '09-下载、校验、修复机制与厂商差异.md' },
  { id: 'prefix', titleKey: 'documents.items.prefix', fallbackTitle: 'Prefix 与模板管理', file: '08-Prefix-与模板管理.md' },
  { id: 'proton', titleKey: 'documents.items.proton', fallbackTitle: 'Proton 下载、管理与使用', file: '03-Proton-下载管理与使用.md' },
  { id: 'dxvk', titleKey: 'documents.items.dxvk', fallbackTitle: 'DXVK / VKD3D 下载、管理与使用', file: '04-DXVK-下载管理与使用.md' },
  { id: 'protection', titleKey: 'documents.items.protection', fallbackTitle: '防护与防封禁管理', file: '05-防护与防封禁管理.md' },
  { id: 'modsWorkflow', titleKey: 'documents.items.modsWorkflow', fallbackTitle: 'Mod 管理与 3DMigoto / Bridge 高级工作流', file: '10-Mod-管理与-3DMigoto-Bridge-高级工作流.md' },
  { id: 'known', titleKey: 'documents.items.known', fallbackTitle: '已知问题与不足', file: '06-已知问题与不足.md' },
  { id: 'troubleshooting', titleKey: 'documents.items.troubleshooting', fallbackTitle: '日志分析与标准排查流程', file: '07-日志分析与标准排查流程.md' },
];

export const builtinDocs: Record<string, string> = Object.fromEntries(
  Object.entries(docModules).map(([path, content]) => {
    const fileName = path.split('/').pop() || path;
    return [fileName, String(content)];
  }),
);
