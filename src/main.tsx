import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import "@fontsource/fira-code/400.css";
import "@fontsource/fira-code/500.css";
import "@fontsource/fira-code/600.css";
import "@fontsource/fira-code/700.css";
import "./shared/styles/base.css";
import "./pages/launcher/launcher.css";
import "./pages/chat/chat.css";
import "./features/git/git.css";
import "./features/messages/messages.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
