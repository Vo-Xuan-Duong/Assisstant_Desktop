import QuickOverlay from "./QuickOverlay";
import QuickRecentResponses from "./QuickRecentResponses";
import QuickResponseActions from "./QuickResponseActions";
import SatelliteDiagnostics from "./SatelliteDiagnostics";
import { useQuickAutoDismiss } from "./quickAutoDismiss";

export default function QuickSurface() {
  useQuickAutoDismiss();
  return (
    <>
      <QuickOverlay />
      <QuickRecentResponses />
      <QuickResponseActions />
      <SatelliteDiagnostics />
    </>
  );
}
