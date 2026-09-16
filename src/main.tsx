import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import DictationOverlay from "./DictationOverlay";
import "./styles.css";

const isOverlay = new URLSearchParams(window.location.search).has("overlay");
createRoot(document.getElementById("root")!).render(
  <StrictMode>{isOverlay ? <DictationOverlay /> : <App />}</StrictMode>
);
