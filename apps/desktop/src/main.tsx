import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import EdgeOverlay from "./EdgeOverlay";
import MainSurface from "./MainSurface";
import QuickOverlay from "./QuickOverlay";

const params = new URLSearchParams(window.location.search);
const surface = params.get("surface");
const isEdgeSurface = surface === "edge";
const isQuickSurface = surface === "quick";

if (isEdgeSurface) {
  document.documentElement.classList.add("edge-surface-root");
  document.body.classList.add("edge-surface-root");
} else if (isQuickSurface) {
  document.documentElement.classList.add("quick-surface-root");
  document.body.classList.add("quick-surface-root");
} else {
  document.documentElement.classList.add("main-surface-root");
  document.body.classList.add("main-surface-root");
}

const surfaceNode = isEdgeSurface ? (
  <EdgeOverlay />
) : isQuickSurface ? (
  <QuickOverlay />
) : (
  <MainSurface />
);

createRoot(document.getElementById("root")!).render(
  <StrictMode>{surfaceNode}</StrictMode>,
);
