import { OverlayToaster, Intent, Position } from "@blueprintjs/core";

let toasterPromise: ReturnType<typeof OverlayToaster.createAsync> | null = null;

export async function showToast(
  message: string,
  intent: "success" | "danger" | "warning" = "success",
): Promise<void> {
  if (typeof window === "undefined") return;

  if (!toasterPromise) {
    toasterPromise = OverlayToaster.createAsync({ position: Position.TOP });
  }

  const toaster = await toasterPromise;
  toaster.show({
    message,
    intent:
      intent === "success" ? Intent.SUCCESS : intent === "danger" ? Intent.DANGER : Intent.WARNING,
    timeout: 4000,
  });
}
