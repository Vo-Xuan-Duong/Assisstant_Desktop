import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import EdgeOverlay from "./EdgeOverlay";
import PermissionSurface from "./PermissionSurface";
import QuickSurface from "./QuickSurface";

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
  document.documentElement.classList.add("permission-surface-root");
  document.body.classList.add("permission-surface-root");
}

const surfaceNode = isEdgeSurface ? (
  <EdgeOverlay />
) : isQuickSurface ? (
  <QuickSurface />
) : (
  <PermissionSurface />
);

createRoot(document.getElementById("root")!).render(
  <StrictMode>{surfaceNode}</StrictMode>,
);
