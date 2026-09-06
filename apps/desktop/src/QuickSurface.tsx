import QuickOverlay from "./QuickOverlay";
import QuickResponseActions from "./QuickResponseActions";
import { useQuickAutoDismiss } from "./quickAutoDismiss";

export default function QuickSurface() {
  useQuickAutoDismiss();
  return (
    <>
      <QuickOverlay />
      <QuickResponseActions />
    </>
  );
}
