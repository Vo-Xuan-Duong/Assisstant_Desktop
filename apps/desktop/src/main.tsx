import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import EdgeOverlay from "./EdgeOverlay";
import MainSurface from "./MainSurface";

const params = new URLSearchParams(window.location.search);
const isEdgeSurface = params.get("surface") === "edge";

createRoot(document.getElementById("root")!).render(
  <StrictMode>{isEdgeSurface ? <EdgeOverlay /> : <MainSurface />}</StrictMode>,
);
