import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import EdgeOverlay from "./EdgeOverlay";
import MainSurface from "./MainSurface";

const params = new URLSearchParams(window.location.search);
const isEdgeSurface = params.get("surface") === "edge";

if (isEdgeSurface) {
  document.documentElement.classList.add("edge-surface-root");
  document.body.classList.add("edge-surface-root");
} else {
  document.documentElement.classList.add("main-surface-root");
  document.body.classList.add("main-surface-root");
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>{isEdgeSurface ? <EdgeOverlay /> : <MainSurface />}</StrictMode>,
);
