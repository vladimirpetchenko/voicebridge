import { useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import LauncherPage from "../pages/launcher/LauncherPage";
import ChatPage from "../pages/chat/ChatPage";

/// Корневой компонент: по метке окна выбирает лаунчер или окно чата.
function App() {
  const [view] = useState(() => {
    try {
      return getCurrentWebviewWindow().label;
    } catch {
      return "main";
    }
  });

  if (view.startsWith("response-")) {
    return <ChatPage />;
  }
  return <LauncherPage />;
}

export default App;
