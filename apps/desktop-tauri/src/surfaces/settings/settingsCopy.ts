import type { SettingsTabId } from "./settingsTabs";

type SettingsLanguage = "system" | "zh-CN" | "en-US";

export interface SettingsCopy {
  title: string;
  close: string;
  navigation: string;
  tabs: Record<SettingsTabId, string>;
  placeholder: string;
  general: {
    title: string;
    autostart: string;
    taskbarTitle: string;
    taskbarDescription: string;
    taskbarEnabled: string;
    taskbarOpacity: string;
    floatBallTitle: string;
    floatBallDescription: string;
    floatBallEnabled: string;
    floatBallOpacity: string;
    floatBallGlow: string;
    refreshInterval: string;
    displayMode: string;
    theme: string;
    language: string;
    refreshOptions: readonly string[];
    displayOptions: readonly string[];
    themeOptions: readonly string[];
    customTheme: string;
    customMode: string;
    customBg: string;
    customSurface: string;
    customFg: string;
    customMuted: string;
    customAccent: string;
    customRadius: string;
    applyCustom: string;
    resetCustom: string;
    system: string;
    simplifiedChinese: string;
  };
  notifications: {
    title: string;
    masterTitle: string;
    masterDescription: string;
    enable: string;
    eventsTitle: string;
    eventsDescription: string;
    warning: string;
    danger: string;
    weeklyReset: string;
    resetCreditIncrease: string;
    refreshFailure: string;
    updateAvailable: string;
    thresholdsTitle: string;
    warningThreshold: string;
    dangerThreshold: string;
    thresholdHelp: string;
    playSound: string;
    sendTest: string;
    testDescription: string;
    testSent: string;
    thresholdInvalid: string;
    saveFailed: string;
    testFailed: string;
  };
  advanced: {
    title: string;
    executablePath: string;
    executablePlaceholder: string;
    validateAndSave: string;
    exportDiagnostics: string;
    compatible: (version: string) => string;
    notFound: string;
    unsupported: string;
    exported: (path: string) => string;
    exportFailed: (error: string) => string;
    unknownVersion: string;
    validationFailed: string;
    exportFailedFriendly: string;
  };
  about: {
    title: string;
    checkForUpdates: string;
    checking: string;
    openReleases: string;
    description: string;
    version: (version: string) => string;
    license: string;
    updateAvailable: (version: string) => string;
    updateCurrent: string;
    updateUnavailable: string;
    updateCheckFailed: string;
  };
  accounts: {
    title: string;
    managed: string;
    signedOut: string;
    selected: string;
    rename: string;
    remove: string;
    renamePrompt: string;
    removeConfirm: (label: string) => string;
    newAccountLabel: string;
    addAccount: string;
  };
  login: {
    dialogLabel: string;
    title: string;
    starting: string;
    returnAfterSignIn: string;
    code: string;
    cancel: string;
    succeeded: string;
    cancelled: string;
    failed: string;
    retryWithDeviceCode: string;
    browser: string;
    deviceCode: string;
    close: string;
  };
}

const english: SettingsCopy = {
  title: "codex-barbar Settings", close: "Close", navigation: "Settings sections",
  tabs: { general: "General", providers: "Accounts", notifications: "Notifications", menuBar: "Menu bar", menu: "Menu", usageSpend: "Usage & spend", advanced: "Advanced", about: "About" },
  placeholder: "This settings section is reserved for a later release.",
  general: {
    title: "General", autostart: "Start at login", taskbarTitle: "Taskbar status", taskbarDescription: "Show a compact usage status in the taskbar.", taskbarEnabled: "Show status in taskbar", taskbarOpacity: "Taskbar status opacity", floatBallTitle: "Floating status ball", floatBallDescription: "Show a movable usage status ball.", floatBallEnabled: "Show floating status ball", floatBallOpacity: "Floating status ball opacity", floatBallGlow: "Floating status ball glow", refreshInterval: "Refresh interval", displayMode: "Display mode", theme: "Theme", language: "Language", refreshOptions: ["Off", "1 minute", "5 minutes", "15 minutes", "30 minutes"], displayOptions: ["Remaining", "Used"], themeOptions: ["System", "Ink Green", "VS Code", "macOS", "Pink", "Blue", "Custom"], customTheme: "Custom skin", customMode: "Mode", customBg: "Background", customSurface: "Surface", customFg: "Text", customMuted: "Muted text", customAccent: "Accent", customRadius: "Corner radius", applyCustom: "Apply custom skin", resetCustom: "Reset custom skin", system: "System", simplifiedChinese: "Simplified Chinese",
  },
  notifications: {
    title: "Notifications",
    masterTitle: "Windows notifications",
    masterDescription: "Opt in to quota, refresh, and release alerts. Existing activity becomes the baseline when enabled.",
    enable: "Enable notifications",
    eventsTitle: "Notify me when",
    eventsDescription: "Choose the changes that deserve a Windows toast.",
    warning: "Remaining quota enters the warning band",
    danger: "Remaining quota enters the danger band",
    weeklyReset: "Universal weekly allowance resets",
    resetCreditIncrease: "Available reset credits increase",
    refreshFailure: "Refresh fails three times or recovers",
    updateAvailable: "A new release is available",
    thresholdsTitle: "Remaining quota thresholds",
    warningThreshold: "Warning remaining percent",
    dangerThreshold: "Danger remaining percent",
    thresholdHelp: "Danger must be lower than warning; both values must be from 0 to 100.",
    playSound: "Play a sound with notifications",
    sendTest: "Send test notification",
    testDescription: "Tests Windows toast delivery without changing usage, reset credits, or account state.",
    testSent: "Test notification sent.",
    thresholdInvalid: "Danger must be lower than warning. Keep both values between 0 and 100.",
    saveFailed: "Notification settings could not be saved. Try again.",
    testFailed: "Windows could not send the test notification. Check notification settings and try again.",
  },
  advanced: { title: "Advanced", executablePath: "Codex executable path", executablePlaceholder: "C:\\Program Files\\Codex\\codex.exe", validateAndSave: "Validate and save", exportDiagnostics: "Export diagnostics", compatible: (version) => `Compatible (${version}).`, notFound: "Codex executable not found.", unsupported: "Unsupported Codex executable.", exported: (path) => `Diagnostics exported to ${path}`, exportFailed: (error) => `Diagnostics export failed: ${error}`, unknownVersion: "unknown version", validationFailed: "Could not validate the Codex executable.", exportFailedFriendly: "Could not export diagnostics." },
  about: { title: "About", checkForUpdates: "Check for updates", checking: "Checking…", openReleases: "Open Releases", description: "codex-barbar – a Windows 11 tray companion for Codex usage.", version: (version) => `Version ${version}`, license: "MIT License. Windows port of CodexBar.", updateAvailable: (version) => `Update available: ${version}`, updateCurrent: "You are on the latest version.", updateUnavailable: "Release feed is unavailable right now.", updateCheckFailed: "Could not check for updates." },
  accounts: { title: "Accounts", managed: "Managed", signedOut: "Signed out", selected: "selected", rename: "Rename", remove: "Remove", renamePrompt: "Rename account", removeConfirm: (label) => `Remove ${label}?`, newAccountLabel: "New account label", addAccount: "Add account" },
  login: { dialogLabel: "Add or re-login account", title: "Account login", starting: "Starting login…", returnAfterSignIn: "Complete the sign-in, then return here.", code: "Code", cancel: "Cancel", succeeded: "Signed in successfully.", cancelled: "Login cancelled.", failed: "Login failed. Try again with a device code.", retryWithDeviceCode: "Retry with device code", browser: "Browser login", deviceCode: "Device code", close: "Close" },
};

const chinese: SettingsCopy = {
  title: "codex-barbar 设置", close: "关闭", navigation: "设置分类",
  tabs: { general: "通用", providers: "账户", notifications: "通知", menuBar: "菜单栏", menu: "菜单", usageSpend: "用量与费用", advanced: "高级", about: "关于" },
  placeholder: "此设置分类将在后续版本中提供。",
  general: {
    title: "通用", autostart: "登录时启动", taskbarTitle: "任务栏状态", taskbarDescription: "在任务栏中显示紧凑的用量状态。", taskbarEnabled: "在任务栏中显示状态", taskbarOpacity: "任务栏状态透明度", floatBallTitle: "悬浮状态球", floatBallDescription: "显示可移动的用量状态球。", floatBallEnabled: "显示悬浮状态球", floatBallOpacity: "悬浮状态球透明度", floatBallGlow: "悬浮状态球荧光亮度", refreshInterval: "刷新间隔", displayMode: "显示模式", theme: "主题", language: "语言", refreshOptions: ["关闭", "1 分钟", "5 分钟", "15 分钟", "30 分钟"], displayOptions: ["剩余", "已使用"], themeOptions: ["系统", "黑绿", "VS Code", "macOS", "粉色", "蓝色", "自定义"], customTheme: "自定义皮肤", customMode: "明暗", customBg: "背景", customSurface: "面板", customFg: "文字", customMuted: "次要文字", customAccent: "强调色", customRadius: "圆角", applyCustom: "应用自定义皮肤", resetCustom: "重置自定义皮肤", system: "系统", simplifiedChinese: "简体中文",
  },
  notifications: {
    title: "通知",
    masterTitle: "Windows 通知",
    masterDescription: "选择接收额度、刷新和版本更新提醒。启用时，当前状态只用于建立基线。",
    enable: "启用通知",
    eventsTitle: "以下情况通知我",
    eventsDescription: "选择需要显示 Windows 通知的变化。",
    warning: "剩余额度进入预警区间",
    danger: "剩余额度进入危险区间",
    weeklyReset: "通用每周额度完成重置",
    resetCreditIncrease: "可用重置额度增加",
    refreshFailure: "刷新连续失败三次或恢复",
    updateAvailable: "有新版本可用",
    thresholdsTitle: "剩余额度阈值",
    warningThreshold: "预警剩余百分比",
    dangerThreshold: "危险剩余百分比",
    thresholdHelp: "危险值必须低于预警值，且两者都应在 0 到 100 之间。",
    playSound: "通知时播放声音",
    sendTest: "发送测试通知",
    testDescription: "测试 Windows 通知，不会更改用量、重置额度或账户状态。",
    testSent: "测试通知已发送。",
    thresholdInvalid: "危险值必须低于预警值。请将两者保持在 0 到 100 之间。",
    saveFailed: "无法保存通知设置，请重试。",
    testFailed: "Windows 无法发送测试通知。请检查通知设置后重试。",
  },
  advanced: { title: "高级", executablePath: "Codex 可执行文件路径", executablePlaceholder: "C:\\Program Files\\Codex\\codex.exe", validateAndSave: "验证并保存", exportDiagnostics: "导出诊断信息", compatible: (version) => `兼容 (${version})。`, notFound: "未找到 Codex 可执行文件。", unsupported: "不支持此 Codex 可执行文件。", exported: (path) => `诊断信息已导出到 ${path}`, exportFailed: (error) => `导出诊断信息失败：${error}`, unknownVersion: "未知版本", validationFailed: "无法验证 Codex 可执行文件。", exportFailedFriendly: "无法导出诊断信息。" },
  about: { title: "关于", checkForUpdates: "检查更新", checking: "正在检查…", openReleases: "打开发布页", description: "codex-barbar 是适用于 Codex 用量的 Windows 11 托盘伴侣。", version: (version) => `当前版本 ${version}`, license: "MIT 许可证。CodexBar 的 Windows 移植版。", updateAvailable: (version) => `有可用更新：${version}`, updateCurrent: "当前已是最新版本。", updateUnavailable: "暂时无法获取发布信息。", updateCheckFailed: "暂时无法检查更新。" },
  accounts: { title: "账户", managed: "托管账户", signedOut: "未登录", selected: "已选择", rename: "重命名", remove: "移除", renamePrompt: "重命名账户", removeConfirm: (label) => `移除 ${label}？`, newAccountLabel: "新账户名称", addAccount: "添加账户" },
  login: { dialogLabel: "添加或重新登录账户", title: "账户登录", starting: "正在开始登录…", returnAfterSignIn: "请完成登录后返回此处。", code: "代码", cancel: "取消", succeeded: "登录成功。", cancelled: "已取消登录。", failed: "登录失败。请使用设备代码重试。", retryWithDeviceCode: "使用设备代码重试", browser: "浏览器登录", deviceCode: "设备代码", close: "关闭" },
};

export function settingsCopy(language: SettingsLanguage): SettingsCopy {
  const isChinese = language === "zh-CN" || (language === "system" && typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh"));
  return isChinese ? chinese : english;
}
