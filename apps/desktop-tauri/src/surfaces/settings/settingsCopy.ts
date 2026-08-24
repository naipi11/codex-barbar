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
  taskbarPresentation: {
    title: string;
    taskbarLegend: string;
    taskbarDescription: string;
    taskbarEnabled: string;
    showIcon: string;
    showAccount: string;
    showWeeklyLabel: string;
    showWeeklyPercent: string;
    showResetDate: string;
    keepOneVisible: string;
    density: string;
    densityOptions: readonly [string, string];
    transparency: string;
    transparencyValue: (value: number) => string;
    transparencyHelp: string;
    transparencySaveFailed: string;
    preferencesSaveFailed: string;
    floatBallLegend: string;
    floatBallDescription: string;
    floatBallEnabled: string;
    floatBallTransparency: string;
    floatBallGlow: string;
    glowValue: (value: number) => string;
    glowSaveFailed: string;
    fullscreenLegend: string;
    fullscreenDescription: string;
    hideInFullscreen: string;
  };
  menu: {
    title: string;
    nativeTrayLegend: string;
    nativeTrayDescription: string;
    trayPanelLegend: string;
    trayPanelDescription: string;
    itemLabels: Record<string, string>;
    visible: string;
    moveUp: string;
    moveDown: string;
    restoreDefaults: string;
    requiredItems: string;
    noCustomCommands: string;
    saveFailed: string;
  };
  usageSpend: {
    title: string;
    officialTitle: string;
    officialDescription: string;
    weeklyAllowance: string;
    remainingPercent: (value: number) => string;
    resetsAt: string;
    lastUpdated: string;
    freshness: Record<"fresh" | "stale" | "missing", string>;
    resetCreditsTitle: string;
    resetCreditsAvailable: (count: number) => string;
    resetCreditsUnsupported: string;
    resetCreditsStale: string;
    localTitle: string;
    localEstimateBadge: string;
    deviceCombined: string;
    rangeLabel: string;
    ranges: readonly [string, string, string, string];
    refreshLocal: string;
    refreshingLocal: string;
    inputTokens: string;
    cachedInputTokens: string;
    outputTokens: string;
    totalTokens: string;
    sessions: string;
    dailyTrendTitle: string;
    dateColumn: string;
    modelTableTitle: string;
    modelColumn: string;
    unknownModelsTitle: string;
    unknownModelsHelp: string;
    malformedSkipped: (count: number) => string;
    emptyState: string;
    unavailableState: string;
    cancelledState: string;
    costUnavailable: string;
    costUsd: (value: number) => string;
    costUnknown: string;
    loading: string;
    loadFailed: string;
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
    resetCreditUnavailable: string;
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
    capabilityAppDisabled: string;
    capabilityGlobalDisabled: string;
    capabilityUnsupported: string;
    openWindowsSettings: string;
    openWindowsSettingsFailed: string;
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
  tabs: { general: "General", providers: "Accounts", notifications: "Notifications", menuBar: "Taskbar & Float Ball", menu: "Panel", usageSpend: "Usage & spend", advanced: "Advanced", about: "About" },
  placeholder: "This settings section is reserved for a later release.",
  general: {
    title: "General", autostart: "Start at login", refreshInterval: "Refresh interval", displayMode: "Display mode", theme: "Theme", language: "Language", refreshOptions: ["Off", "1 minute", "5 minutes", "15 minutes", "30 minutes"], displayOptions: ["Remaining", "Used"], themeOptions: ["System", "Ink Green", "VS Code", "macOS", "Pink", "Blue", "Custom"], customTheme: "Custom skin", customMode: "Mode", customBg: "Background", customSurface: "Surface", customFg: "Text", customMuted: "Muted text", customAccent: "Accent", customRadius: "Corner radius", applyCustom: "Apply custom skin", resetCustom: "Reset custom skin", system: "System", simplifiedChinese: "Simplified Chinese",
  },
  taskbarPresentation: {
    title: "Taskbar & Float Ball",
    taskbarLegend: "Taskbar status",
    taskbarDescription: "Choose the compact usage readout that stays visible while you work.",
    taskbarEnabled: "Show taskbar status",
    showIcon: "Show product icon",
    showAccount: "Show account name",
    showWeeklyLabel: "Show weekly label",
    showWeeklyPercent: "Show remaining percentage",
    showResetDate: "Show reset date",
    keepOneVisible: "Keep at least one taskbar item visible while taskbar status is on.",
    density: "Density",
    densityOptions: ["Compact", "Standard"],
    transparency: "Transparency",
    transparencyValue: (value) => `${value}% transparent`,
    transparencyHelp: "0% is most opaque; 100% is most transparent.",
    transparencySaveFailed: "Transparency could not be saved. Try again.",
    preferencesSaveFailed: "Taskbar and float ball settings could not be saved. Try again.",
    floatBallLegend: "Floating status ball",
    floatBallDescription: "Show a movable usage status ball.",
    floatBallEnabled: "Show floating status ball",
    floatBallTransparency: "Floating status ball transparency",
    floatBallGlow: "Floating status ball glow",
    glowValue: (value) => `${value}% brightness`,
    glowSaveFailed: "Glow could not be saved. Try again.",
    fullscreenLegend: "Full-screen behavior",
    fullscreenDescription: "Hide glanceable status surfaces over full-screen apps without removing the native tray icon.",
    hideInFullscreen: "Hide status surfaces during full-screen apps",
  },
  menu: {
    title: "Panel",
    nativeTrayLegend: "Tray menu",
    nativeTrayDescription:
      "Choose which built-in items appear in the tray right-click menu and in what order. Settings and Quit are always available.",
    trayPanelLegend: "Panel quick actions",
    trayPanelDescription:
      "Choose which quick actions appear in the tray panel and in what order.",
    itemLabels: {
      open_panel: "Open codex-barbar",
      refresh: "Refresh",
      accounts: "Accounts",
      open_usage: "Open Codex Usage",
      settings: "Settings",
      about: "About",
      quit: "Quit",
      dismiss: "Dismiss",
    },
    visible: "Visible",
    moveUp: "Move up",
    moveDown: "Move down",
    restoreDefaults: "Restore defaults",
    requiredItems: "Settings and Quit are required and cannot be hidden.",
    noCustomCommands:
      "Only built-in items can be configured. Custom commands, scripts, URLs, and executable paths are not supported.",
    saveFailed: "Menu settings could not be saved. Try again.",
  },

  usageSpend: {
    title: "Usage & Spend",
    officialTitle: "Official weekly allowance",
    officialDescription:
      "Read-only view of the universal weekly Codex allowance from the selected account.",
    weeklyAllowance: "Universal weekly allowance",
    remainingPercent: (value) => `${value}% remaining`,
    resetsAt: "Resets",
    lastUpdated: "Last updated",
    freshness: {
      fresh: "Fresh",
      stale: "Stale",
      missing: "No data",
    },
    resetCreditsTitle: "Reset credits",
    resetCreditsAvailable: (count) => `${count} reset credit(s) available`,
    resetCreditsUnsupported: "Reset credits are not reported for this account.",
    resetCreditsStale: "Reset-credit count is from a cached snapshot.",
    localTitle: "Local usage estimate",
    localEstimateBadge: "Local estimate, not an OpenAI bill",
    deviceCombined: "This device combined",
    rangeLabel: "Local range",
    ranges: ["Today", "Last 7 days", "Last 30 days", "Current weekly"],
    refreshLocal: "Refresh local data",
    refreshingLocal: "Refreshing local data…",
    inputTokens: "Input tokens",
    cachedInputTokens: "Cached input tokens",
    outputTokens: "Output tokens",
    totalTokens: "Total tokens",
    sessions: "Local sessions",
    dailyTrendTitle: "Daily trend",
    dateColumn: "Date",
    modelTableTitle: "Per-model totals",
    modelColumn: "Model",
    unknownModelsTitle: "Unpriced models",
    unknownModelsHelp:
      "These models contributed tokens but have no known price, so no aggregate cost is shown.",
    malformedSkipped: (count) => `${count} malformed log line(s) skipped`,
    emptyState: "No local Codex session logs found in this range.",
    unavailableState: "Local usage is unavailable for this range.",
    cancelledState: "Local scan was cancelled.",
    costUnavailable: "Cost unavailable",
    costUsd: (value) => `$${value.toFixed(2)}`,
    costUnknown: "—",
    loading: "Loading usage data…",
    loadFailed: "Usage data could not be loaded. Try again.",
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
    resetCreditUnavailable: "Reset-credit notifications are not available yet. Usage history support will enable them later.",
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
    capabilityAppDisabled: "Notifications for codex-barbar are turned off in Windows. Open Windows notification settings and allow notifications for codex-barbar.",
    capabilityGlobalDisabled: "Windows notifications are turned off. Open Windows notification settings to turn them on.",
    capabilityUnsupported: "Windows notification availability could not be checked on this system.",
    openWindowsSettings: "Open Windows notification settings",
    openWindowsSettingsFailed: "Windows notification settings could not be opened. Open Settings > System > Notifications manually.",
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
  tabs: { general: "通用", providers: "账户", notifications: "通知", menuBar: "任务栏与悬浮球", menu: "面板", usageSpend: "用量与费用", advanced: "高级", about: "关于" },
  placeholder: "此设置分类将在后续版本中提供。",
  general: {
    title: "通用", autostart: "登录时启动", refreshInterval: "刷新间隔", displayMode: "显示模式", theme: "主题", language: "语言", refreshOptions: ["关闭", "1 分钟", "5 分钟", "15 分钟", "30 分钟"], displayOptions: ["剩余", "已使用"], themeOptions: ["系统", "黑绿", "VS Code", "macOS", "粉色", "蓝色", "自定义"], customTheme: "自定义皮肤", customMode: "明暗", customBg: "背景", customSurface: "面板", customFg: "文字", customMuted: "次要文字", customAccent: "强调色", customRadius: "圆角", applyCustom: "应用自定义皮肤", resetCustom: "重置自定义皮肤", system: "系统", simplifiedChinese: "简体中文",
  },
  taskbarPresentation: {
    title: "任务栏与悬浮球",
    taskbarLegend: "任务栏状态",
    taskbarDescription: "选择工作时持续显示的紧凑用量信息。",
    taskbarEnabled: "显示任务栏状态",
    showIcon: "显示产品图标",
    showAccount: "显示账户名称",
    showWeeklyLabel: "显示每周标签",
    showWeeklyPercent: "显示剩余百分比",
    showResetDate: "显示重置日期",
    keepOneVisible: "任务栏状态开启时，请至少保留一个可见项目。",
    density: "密度",
    densityOptions: ["紧凑", "标准"],
    transparency: "透明度",
    transparencyValue: (value) => `${value}% 透明`,
    transparencyHelp: "0% 最不透明，100% 最透明。",
    transparencySaveFailed: "无法保存透明度，请重试。",
    preferencesSaveFailed: "无法保存任务栏与悬浮球设置，请重试。",
    floatBallLegend: "悬浮状态球",
    floatBallDescription: "显示可移动的用量状态球。",
    floatBallEnabled: "显示悬浮状态球",
    floatBallTransparency: "悬浮状态球透明度",
    floatBallGlow: "悬浮状态球荧光亮度",
    glowValue: (value) => `${value}% 亮度`,
    glowSaveFailed: "无法保存荧光亮度，请重试。",
    fullscreenLegend: "全屏行为",
    fullscreenDescription: "全屏应用运行时隐藏状态界面，但保留原生托盘图标。",
    hideInFullscreen: "全屏应用运行时隐藏状态界面",
  },
  menu: {
    title: "面板",
    nativeTrayLegend: "托盘菜单",
    nativeTrayDescription:
      "选择托盘右键菜单中显示的项及其顺序。设置与退出始终可用。",
    trayPanelLegend: "面板快捷操作",
    trayPanelDescription: "选择托盘面板中显示的快捷操作及其顺序。",
    itemLabels: {
      open_panel: "打开 codex-barbar",
      refresh: "刷新",
      accounts: "账户",
      open_usage: "打开 Codex 用量",
      settings: "设置",
      about: "关于",
      quit: "退出",
      dismiss: "关闭",
    },
    visible: "可见",
    moveUp: "上移",
    moveDown: "下移",
    restoreDefaults: "恢复默认",
    requiredItems: "设置与退出为必选项，无法隐藏。",
    noCustomCommands: "仅可配置内置项，不支持自定义命令、脚本、网址或可执行文件。",
    saveFailed: "无法保存菜单设置，请重试。",
  },

  usageSpend: {
    title: "用量与费用",
    officialTitle: "官方每周额度",
    officialDescription: "所选账户的通用每周 Codex 额度，只读展示。",
    weeklyAllowance: "通用每周额度",
    remainingPercent: (value) => `剩余 ${value}%`,
    resetsAt: "重置时间",
    lastUpdated: "最后更新",
    freshness: {
      fresh: "最新",
      stale: "已过期",
      missing: "暂无数据",
    },
    resetCreditsTitle: "重置额度",
    resetCreditsAvailable: (count) => `可用重置额度 ${count} 个`,
    resetCreditsUnsupported: "当前账户未返回重置额度信息。",
    resetCreditsStale: "重置额度来自缓存快照。",
    localTitle: "本地用量估算",
    localEstimateBadge: "本地估算，并非 OpenAI 账单",
    deviceCombined: "此设备合计",
    rangeLabel: "本地统计范围",
    ranges: ["今天", "最近 7 天", "最近 30 天", "当前每周"],
    refreshLocal: "刷新本地数据",
    refreshingLocal: "正在刷新本地数据…",
    inputTokens: "输入令牌",
    cachedInputTokens: "缓存输入令牌",
    outputTokens: "输出令牌",
    totalTokens: "令牌总数",
    sessions: "本地会话",
    dailyTrendTitle: "每日趋势",
    dateColumn: "日期",
    modelTableTitle: "按模型统计",
    modelColumn: "模型",
    unknownModelsTitle: "未定价模型",
    unknownModelsHelp: "这些模型产生了令牌但无已知价格，因此不显示总费用。",
    malformedSkipped: (count) => `跳过 ${count} 行格式异常日志`,
    emptyState: "该范围内未找到本地 Codex 会话日志。",
    unavailableState: "该范围暂无法提供本地用量。",
    cancelledState: "本地扫描已取消。",
    costUnavailable: "费用不可用",
    costUsd: (value) => `$${value.toFixed(2)}`,
    costUnknown: "—",
    loading: "正在加载用量数据…",
    loadFailed: "无法加载用量数据，请重试。",
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
    resetCreditUnavailable: "重置额度通知暂不可用，后续用量历史功能将启用此选项。",
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
    capabilityAppDisabled: "Windows 已关闭 codex-barbar 的通知。请打开 Windows 通知设置，并允许 codex-barbar 发送通知。",
    capabilityGlobalDisabled: "Windows 通知已关闭。请打开 Windows 通知设置并启用通知。",
    capabilityUnsupported: "无法在此系统上检查 Windows 通知是否可用。",
    openWindowsSettings: "打开 Windows 通知设置",
    openWindowsSettingsFailed: "无法打开 Windows 通知设置。请手动打开“设置 > 系统 > 通知”。",
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
