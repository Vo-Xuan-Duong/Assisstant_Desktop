import QuickOverlay from "./QuickOverlay";
import { useQuickAutoDismiss } from "./quickAutoDismiss";

export default function QuickSurface() {
  useQuickAutoDismiss();
  return <QuickOverlay />;
}
