import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import TrayPanel from "./surfaces/TrayPanel";
import Settings from "./surfaces/Settings";
import TaskbarStatus from "./surfaces/TaskbarStatus";
import TaskbarStatusMeasure from "./surfaces/TaskbarStatusMeasure";
import FloatBall from "./surfaces/FloatBall";

/**
 * Route only the shipping V1 surfaces.  The hidden `main` WebView is the
 * tray panel host and `settings` is the sole detached window.  Legacy
 * auxiliary labels intentionally render nothing.
 */
export default function App() {
  const label = getCurrentWebviewWindow().label;

  if (label === "settings") {
    return <Settings />;
  }
  if (label === "main") {
    return <TrayPanel />;
  }
  if (label === "taskbar-status") {
    return <TaskbarStatus />;
  }
  if (label === "taskbar-status-measure") {
    return <TaskbarStatusMeasure />;
  }
  if (label === "float-ball") {
    return <FloatBall />;
  }
  return null;
}
