import QuickOverlay from "./QuickOverlay";
import QuickRecentResponses from "./QuickRecentResponses";
import QuickResponseActions from "./QuickResponseActions";
import { useQuickAutoDismiss } from "./quickAutoDismiss";

export default function QuickSurface() {
  useQuickAutoDismiss();
  return (
    <>
      <QuickOverlay />
      <QuickRecentResponses />
      <QuickResponseActions />
    </>
  );
}
