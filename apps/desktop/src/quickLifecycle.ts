export const QUICK_AUTO_DISMISS_HOLD_EVENT = "quick:auto-dismiss-hold";

export interface QuickAutoDismissHoldDetail {
  source: string;
  held: boolean;
}

export function setQuickAutoDismissHold(source: string, held: boolean) {
  window.dispatchEvent(
    new CustomEvent<QuickAutoDismissHoldDetail>(QUICK_AUTO_DISMISS_HOLD_EVENT, {
      detail: { source, held },
    }),
  );
}
