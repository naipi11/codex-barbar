import type { AppErrorKind, RecoveryAction } from "../../types/bridge";

export interface TrayCopy {
  account: string;
  profile: string;
  noProfiles: string;
  currentCli: string;
  signedOut: string;
  managed: string;
  remaining: string;
  used: string;
  refreshing: string;
  switching: string;
  awaitingRefresh: string;
  lastUpdated: string;
  cached: string;
  fresh: string;
  missing: string;
  protocolAnomaly: string;
  refresh: string;
  openUsage: string;
  settings: string;
  hidePanel: string;
  dismiss: string;
  quit: string;
  actions: string;
  dataStatus: string;
  retry: string;
  signIn: string;
  reLogin: string;
  installTestedCodex: string;
  selectExecutable: string;
  waitAndRetry: string;
  explainApiBilling: string;
  exportDiagnostics: string;
  errorMessages: Record<AppErrorKind, string>;
  actionLabels: Record<RecoveryAction, string>;
}

const english: TrayCopy = {
  account: "Account",
  profile: "Profile",
  noProfiles: "No profiles",
  currentCli: "Current CLI",
  signedOut: "Signed out",
  managed: "Managed",
  remaining: "remaining",
  used: "used",
  refreshing: "Refreshing…",
  switching: "Switching profile…",
  awaitingRefresh: "Awaiting refresh",
  lastUpdated: "Last updated",
  cached: "Cached",
  fresh: "Updated",
  missing: "No usage data",
  protocolAnomaly: "Some Codex usage fields were normalized.",
  refresh: "Refresh",
  openUsage: "Usage",
  settings: "Settings",
  hidePanel: "Hide panel",
  dismiss: "Dismiss",
  quit: "Quit",
  actions: "Actions",
  dataStatus: "Data status",
  retry: "Retry",
  signIn: "Sign in",
  reLogin: "Re-login",
  installTestedCodex: "View tested Codex versions",
  selectExecutable: "Select Codex executable",
  waitAndRetry: "Try again later",
  explainApiBilling: "Open usage",
  exportDiagnostics: "Export diagnostics",
  errorMessages: {
    codexNotFound: "Codex was not found.",
    unsupportedCodexVersion: "This Codex version is not supported.",
    notSignedIn: "Codex is not signed in.",
    apiKeyNoQuota: "API-key accounts do not have ChatGPT quota.",
    authExpired: "Sign-in expired.",
    offlineOrTimeout: "Offline or timed out.",
    rateLimited: "The service is rate-limiting requests.",
    protocolMismatch: "The Codex protocol is incompatible.",
    vaultFailure: "The managed account vault could not be opened.",
    storageFailure: "Local storage is unavailable.",
  },
  actionLabels: {
    selectCodexExecutable: "Select Codex executable",
    installTestedCodex: "View tested Codex versions",
    signIn: "Sign in",
    reloginManagedProfile: "Re-login",
    retry: "Retry",
    waitAndRetry: "Try again later",
    explainApiBilling: "Open usage",
    exportDiagnostics: "Export diagnostics",
  },
};

const chinese: TrayCopy = {
  ...english,
  account: "账户",
  profile: "账户配置",
  noProfiles: "暂无账户",
  currentCli: "当前 CLI",
  signedOut: "未登录",
  managed: "应用账户",
  remaining: "剩余",
  used: "已使用",
  refreshing: "正在刷新…",
  switching: "正在切换账户…",
  awaitingRefresh: "等待刷新",
  lastUpdated: "最后更新",
  cached: "缓存",
  fresh: "已更新",
  missing: "暂无用量数据",
  protocolAnomaly: "部分额度字段已按 Codex 返回值校正。",
  refresh: "刷新",
  openUsage: "用量",
  settings: "设置",
  hidePanel: "隐藏面板",
  dismiss: "关闭",
  quit: "退出",
  actions: "操作",
  dataStatus: "数据状态",
  retry: "重试",
  signIn: "登录",
  reLogin: "重新登录",
  installTestedCodex: "查看已测试的 Codex 版本",
  selectExecutable: "选择 Codex 可执行文件",
  waitAndRetry: "稍后重试",
  explainApiBilling: "打开用量",
  exportDiagnostics: "导出诊断",
  errorMessages: {
    codexNotFound: "未找到 Codex。",
    unsupportedCodexVersion: "当前 Codex 版本不受支持。",
    notSignedIn: "Codex 尚未登录。",
    apiKeyNoQuota: "API Key 账户没有 ChatGPT 套餐额度。",
    authExpired: "登录已过期。",
    offlineOrTimeout: "网络不可用或请求超时。",
    rateLimited: "请求过于频繁。",
    protocolMismatch: "Codex 协议版本不兼容。",
    vaultFailure: "无法打开应用账户保险库。",
    storageFailure: "本地存储不可用。",
  },
  actionLabels: {
    selectCodexExecutable: "选择 Codex 可执行文件",
    installTestedCodex: "查看已测试的 Codex 版本",
    signIn: "登录",
    reloginManagedProfile: "重新登录",
    retry: "重试",
    waitAndRetry: "稍后重试",
    explainApiBilling: "打开用量",
    exportDiagnostics: "导出诊断",
  },
};

export function trayCopy(language: "system" | "zh-CN" | "en-US"): TrayCopy {
  if (
    language === "zh-CN" ||
    (language === "system" &&
      typeof navigator !== "undefined" &&
      navigator.language.toLowerCase().startsWith("zh"))
  ) {
    return chinese;
  }
  return english;
}

export function windowLabel(
  label: string | null,
  durationMinutes: number | null,
  locale: string,
): string {
  if (durationMinutes === 300) return locale.startsWith("zh") ? "5小时额度" : "5-hour quota";
  if (durationMinutes === 10_080) return locale.startsWith("zh") ? "每周额度" : "Weekly quota";
  if (label && !label.startsWith("usage.")) return label;
  if (durationMinutes !== null && Number.isFinite(durationMinutes)) {
    return locale.startsWith("zh")
      ? `${Math.round(durationMinutes)}分钟额度`
      : `${Math.round(durationMinutes)} minutes quota`;
  }
  return locale.startsWith("zh") ? "额度" : "Quota";
}
